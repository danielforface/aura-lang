#![forbid(unsafe_code)]

use miette::{Diagnostic, IntoDiagnostic};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("AI optimizer error: {message}")]
#[diagnostic(code(aura::ai_opt))]
pub struct AiOptError {
    pub message: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HostFeatures {
    pub avx512: bool,
    pub amx: bool,
}

/// Injects a small amount of LLVM metadata.
///
/// Current implementation is intentionally conservative:
/// - Adds module flags that enable loop vectorization hints.
/// - If the IR contains Aura markers, it can rewrite them into LLVM metadata.
pub fn optimize_ll(input_ll: &str, host: HostFeatures) -> Result<String, AiOptError> {
    let mut lines: Vec<String> = input_ll.lines().map(|l| l.to_string()).collect();

    // === Pass 1: discover existing metadata IDs and module flags ===
    let max_md = max_metadata_id(input_ll);
    let mut next_md = max_md.map(|n| n + 1).unwrap_or(0);
    let has_module_flags = input_ll.contains("!llvm.module.flags");

    // === Pass 2.5: tag AI inference calls ===
    // We use a custom per-instruction metadata attachment so downstream tooling
    // can reliably spot inference sites.
    let mut infer_tag_id: Option<u32> = None;

    // === Pass 2: lightweight value flow: tensor handle -> constant length ===
    let tensor_len_map = infer_const_tensor_lengths(&lines);

    // === Pass 3: rewrite call sites with !range when we can prove an exact length ===
    let mut range_nodes: Vec<(u32, i64, i64)> = Vec::new();
    let mut range_cache: std::collections::HashMap<(i64, i64), u32> = std::collections::HashMap::new();

    for line in &mut lines {
        if let Some((dest, arg)) = parse_tensor_len_call(line) {
            if let Some(len) = tensor_len_map.get(&arg).copied() {
                // Exact constant: [len, len+1)
                let lo = len;
                let hi = len.saturating_add(1);
                let md_id = *range_cache.entry((lo, hi)).or_insert_with(|| {
                    let id = next_md;
                    next_md += 1;
                    range_nodes.push((id, lo, hi));
                    id
                });

                // Attach metadata unless already present.
                if !line.contains("!range") {
                    *line = attach_metadata_before_comment(line, &format!("!range !{md_id}"));
                }

                // Suppress unused warning for dest, but keep parser strict.
                let _ = dest;
            }
        }

        // Tag:
        //   %x = call i32 @aura_ai_infer(i32 %m, i32 %t)
        // with:
        //   %x = call i32 @aura_ai_infer(...) , !aura.ai.infer !N
        let (code, _comment) = split_comment(line);
        if code.contains("@aura_ai_infer") && code.contains("call") {
            if !line.contains("!aura.ai.infer") {
                let md_id = *infer_tag_id.get_or_insert_with(|| {
                    let id = next_md;
                    next_md += 1;
                    id
                });
                *line = attach_metadata_before_comment(line, &format!("!aura.ai.infer !{md_id}"));
            }
        }
    }

    // === Footer ===
    let mut out = String::with_capacity(input_ll.len() + 1024);
    for l in &lines {
        out.push_str(l);
        out.push('\n');
    }

    out.push_str("\n; === aura-ai-opt metadata footer ===\n");
    if host.avx512 {
        out.push_str("; aura.host.avx512 = true\n");
    }
    if host.amx {
        out.push_str("; aura.host.amx = true\n");
    }

    if !has_module_flags {
        let flag_id = next_md;
        out.push_str(&format!("!llvm.module.flags = !{{!{flag_id}}}\n"));
        out.push_str(&format!(
            "!{flag_id} = !{{i32 2, !\"Debug Info Version\", i32 3}}\n"
        ));
    }

    for (id, lo, hi) in range_nodes {
        out.push_str(&format!("!{id} = !{{i32 {lo}, i32 {hi}}}\n"));
    }

    if let Some(id) = infer_tag_id {
        out.push_str(&format!("!{id} = !{{!\"aura.ai.infer\"}}\n"));
    }

    Ok(out)
}

pub fn host_features() -> HostFeatures {
    // Only meaningful on x86_64; on other platforms just return false.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        HostFeatures {
            avx512: std::arch::is_x86_feature_detected!("avx512f"),
            // NOTE: AMX feature detection currently requires unstable Rust intrinsics.
            // Keep the field for future work, but default it to false on stable.
            amx: false,
        }
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        HostFeatures::default()
    }
}

pub fn optimize_ll_file(input: &std::path::Path, output: &std::path::Path) -> miette::Result<()> {
    let ll = std::fs::read_to_string(input).into_diagnostic()?;
    let optimized = optimize_ll(&ll, host_features()).map_err(miette::Report::new)?;
    std::fs::write(output, optimized).into_diagnostic()?;
    Ok(())
}

fn max_metadata_id(text: &str) -> Option<u32> {
    let mut max_id: Option<u32> = None;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'!' {
            let mut j = i + 1;
            let mut n: u32 = 0;
            let mut any = false;
            while j < bytes.len() {
                let c = bytes[j];
                if c.is_ascii_digit() {
                    any = true;
                    n = n.saturating_mul(10).saturating_add((c - b'0') as u32);
                    j += 1;
                } else {
                    break;
                }
            }
            if any {
                max_id = Some(max_id.map(|m| m.max(n)).unwrap_or(n));
            }
            i = j;
            continue;
        }
        i += 1;
    }
    max_id
}

fn infer_const_tensor_lengths(lines: &[String]) -> std::collections::HashMap<String, i64> {
    // Tracks `%t` -> 100 when we see: `%t = call i32 @aura_tensor_new(i32 100)`
    let mut out: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for line in lines {
        let Some((dest, len)) = parse_tensor_new_call(line) else {
            continue;
        };
        out.insert(dest, len);
    }
    out
}

fn parse_tensor_new_call(line: &str) -> Option<(String, i64)> {
    // Very small recognizer for:
    //   %t = call i32 @aura_tensor_new(i32 100)
    //   %t = call i32 @aura_tensor_new(i64 100)
    let (code, _) = split_comment(line);
    let code = code.trim();
    if !code.contains("@aura_tensor_new") {
        return None;
    }
    let eq = code.find('=')?;
    let dest = code[..eq].trim();
    if !dest.starts_with('%') {
        return None;
    }
    // Find the integer literal argument inside the parens.
    let open = code.find('(')?;
    let close = code.rfind(')')?;
    let args = &code[open + 1..close];
    let lit = extract_typed_int_literal(args)?;
    Some((dest.to_string(), lit))
}

fn parse_tensor_len_call(line: &str) -> Option<(String, String)> {
    // Recognize:
    //   %len = call i32 @aura_tensor_len(i32 %t)
    let (code, _) = split_comment(line);
    let code = code.trim();
    if !code.contains("@aura_tensor_len") {
        return None;
    }
    let eq = code.find('=')?;
    let dest = code[..eq].trim();
    if !dest.starts_with('%') {
        return None;
    }
    let open = code.find('(')?;
    let close = code.rfind(')')?;
    let args = &code[open + 1..close];
    let arg = extract_first_ssa_value(args)?;
    Some((dest.to_string(), arg))
}

fn extract_typed_int_literal(args: &str) -> Option<i64> {
    // LLVM syntax examples we care about:
    //   i32 100
    //   i64 -1
    //   i32 0
    // This intentionally ignores the bitwidth token (i32/i64) and reads the
    // numeric literal that follows it.
    let toks: Vec<&str> = args
        .split(|c: char| c == ',' || c == '(' || c == ')')
        .flat_map(|chunk| chunk.split_whitespace())
        .collect();

    if toks.len() >= 2 && looks_like_int_type(toks[0]) {
        return parse_int_token(toks[1]);
    }

    // Fallback: last numeric token.
    for t in toks.iter().rev() {
        if let Some(n) = parse_int_token(t) {
            return Some(n);
        }
    }
    None
}

fn looks_like_int_type(tok: &str) -> bool {
    let Some(rest) = tok.strip_prefix('i') else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
}

fn parse_int_token(tok: &str) -> Option<i64> {
    let t = tok.trim();
    if t.is_empty() {
        return None;
    }
    // Disallow tokens like "%v3".
    if t.starts_with('%') {
        return None;
    }
    t.parse::<i64>().ok()
}

fn extract_first_ssa_value(s: &str) -> Option<String> {
    // Find first "%name" token.
    for tok in s.split(|c: char| c.is_whitespace() || c == ',' || c == ')') {
        let t = tok.trim();
        if t.starts_with('%') && t.len() > 1 {
            return Some(t.to_string());
        }
    }
    None
}

fn split_comment(line: &str) -> (&str, Option<&str>) {
    match line.find(';') {
        Some(i) => (&line[..i], Some(&line[i..])),
        None => (line, None),
    }
}

fn attach_metadata_before_comment(line: &str, md: &str) -> String {
    let (code, comment) = split_comment(line);
    let code = code.trim_end();
    let mut out = String::with_capacity(line.len() + md.len() + 4);
    out.push_str(code);
    out.push_str(", ");
    out.push_str(md);
    if let Some(c) = comment {
        out.push(' ');
        out.push_str(c.trim_start());
    }
    out
}
