#![forbid(unsafe_code)]

use std::path::Path;

use aura_ast::{ExternCell, Program, Span, Stmt, UnsafeBlock};
use miette::IntoDiagnostic;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SpanRange {
    pub offset: usize,
    pub len: usize,
}

impl From<Span> for SpanRange {
    fn from(s: Span) -> Self {
        Self {
            offset: s.offset().into(),
            len: s.len().into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustedCoreReport {
    pub schema: &'static str,
    pub input: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub trusted: TrustedSurface,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify: Option<VerifyEvidenceReport>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct VerifyEvidenceReport {
    pub assumptions: Vec<SpanRange>,
    pub obligations: Vec<SpanRange>,
    pub proofs: ProofSummary,
    pub by_cell: Vec<CellProofSummary>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProofSummary {
    pub total: usize,
    pub by_plugin: Vec<CountBy>,
    pub by_kind: Vec<CountBy>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CountBy {
    pub key: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CellProofSummary {
    pub cell: String,
    pub span: SpanRange,
    pub proofs: ProofSummary,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TrustedSurface {
    pub unsafe_blocks: Vec<SpanRange>,
    pub extern_cells_trusted: Vec<SpanRange>,
    pub extern_cells_untrusted: Vec<SpanRange>,
}

pub fn analyze_trusted_surface(program: &Program) -> TrustedSurface {
    let mut out = TrustedSurface::default();
    for s in &program.stmts {
        analyze_stmt(&mut out, s);
    }
    out
}

fn analyze_stmt(out: &mut TrustedSurface, stmt: &Stmt) {
    match stmt {
        Stmt::ExternCell(ExternCell { span, trusted, .. }) => {
            if *trusted {
                out.extern_cells_trusted.push((*span).into());
            } else {
                out.extern_cells_untrusted.push((*span).into());
            }
        }
        Stmt::UnsafeBlock(UnsafeBlock { span, body }) => {
            out.unsafe_blocks.push((*span).into());
            for s in &body.stmts {
                analyze_stmt(out, s);
            }
        }
        Stmt::If(s) => {
            for st in &s.then_block.stmts {
                analyze_stmt(out, st);
            }
            if let Some(else_block) = &s.else_block {
                for st in &else_block.stmts {
                    analyze_stmt(out, st);
                }
            }
        }
        Stmt::Match(s) => {
            for a in &s.arms {
                for st in &a.body.stmts {
                    analyze_stmt(out, st);
                }
            }
        }
        Stmt::While(s) => {
            for st in &s.body.stmts {
                analyze_stmt(out, st);
            }
        }
        Stmt::Layout(s) => {
            for st in &s.body.stmts {
                analyze_stmt(out, st);
            }
        }
        Stmt::FlowBlock(s) => {
            for st in &s.body.stmts {
                analyze_stmt(out, st);
            }
        }
        _ => {}
    }
}

pub fn write_verify_report(
    path: &Path,
    ok: bool,
    error: Option<String>,
    program: Option<&Program>,
    verify: Option<VerifyEvidenceReport>,
    out_path: &Path,
) -> miette::Result<()> {
    let report = TrustedCoreReport {
        schema: "aura.trusted-core.v1",
        input: display_path(path),
        ok,
        error,
        trusted: program
            .map(analyze_trusted_surface)
            .unwrap_or_else(TrustedSurface::default),
        verify,
    };

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).into_diagnostic()?;
    }
    let json = serde_json::to_string_pretty(&report).into_diagnostic()?;
    std::fs::write(out_path, json).into_diagnostic()?;
    Ok(())
}

pub fn analyze_verify_surface(program: &Program) -> (Vec<SpanRange>, Vec<SpanRange>) {
    let mut assumptions = Vec::new();
    let mut obligations = Vec::new();
    for s in &program.stmts {
        analyze_verify_stmt(&mut assumptions, &mut obligations, s);
    }
    (assumptions, obligations)
}

fn analyze_verify_stmt(assumptions: &mut Vec<SpanRange>, obligations: &mut Vec<SpanRange>, stmt: &Stmt) {
    match stmt {
        Stmt::Requires(s) => assumptions.push(s.span.into()),
        Stmt::Assume(s) => assumptions.push(s.span.into()),
        Stmt::Ensures(s) => obligations.push(s.span.into()),
        Stmt::Assert(s) => obligations.push(s.span.into()),
        Stmt::CellDef(c) => {
            for st in &c.body.stmts {
                analyze_verify_stmt(assumptions, obligations, st);
            }
        }
        Stmt::UnsafeBlock(UnsafeBlock { body, .. }) => {
            for st in &body.stmts {
                analyze_verify_stmt(assumptions, obligations, st);
            }
        }
        Stmt::If(s) => {
            for st in &s.then_block.stmts {
                analyze_verify_stmt(assumptions, obligations, st);
            }
            if let Some(else_block) = &s.else_block {
                for st in &else_block.stmts {
                    analyze_verify_stmt(assumptions, obligations, st);
                }
            }
        }
        Stmt::Match(s) => {
            for a in &s.arms {
                for st in &a.body.stmts {
                    analyze_verify_stmt(assumptions, obligations, st);
                }
            }
        }
        Stmt::While(s) => {
            // Invariants and decreases are expressions; proof notes capture the details.
            for st in &s.body.stmts {
                analyze_verify_stmt(assumptions, obligations, st);
            }
        }
        Stmt::Layout(s) => {
            for st in &s.body.stmts {
                analyze_verify_stmt(assumptions, obligations, st);
            }
        }
        Stmt::FlowBlock(s) => {
            for st in &s.body.stmts {
                analyze_verify_stmt(assumptions, obligations, st);
            }
        }
        _ => {}
    }
}

#[cfg(feature = "z3")]
pub fn analyze_verify_evidence(program: &Program, proofs: &[aura_nexus::ProofNote]) -> VerifyEvidenceReport {
    let (assumptions, obligations) = analyze_verify_surface(program);

    let proofs_summary = summarize_proofs(proofs);
    let mut by_cell: Vec<CellProofSummary> = Vec::new();

    // Build a list of cell spans to categorize proof notes.
    let mut cells: Vec<(String, SpanRange, Vec<&aura_nexus::ProofNote>)> = Vec::new();
    for s in &program.stmts {
        if let Stmt::CellDef(c) = s {
            cells.push((c.name.node.clone(), c.span.into(), Vec::new()));
        }
    }

    for p in proofs {
        let poff: usize = p.span.offset().into();
        let mut assigned = false;
        for (_name, span, notes) in &mut cells {
            let start = span.offset;
            let end = span.offset.saturating_add(span.len);
            if poff >= start && poff < end {
                notes.push(p);
                assigned = true;
                break;
            }
        }
        if !assigned {
            // Drop on the floor for now; callers can still inspect full list in editor.
        }
    }

    for (name, span, notes) in cells {
        by_cell.push(CellProofSummary {
            cell: name,
            span,
            proofs: summarize_proofs(&notes.into_iter().cloned().collect::<Vec<_>>()),
        });
    }

    VerifyEvidenceReport {
        assumptions,
        obligations,
        proofs: proofs_summary,
        by_cell,
    }
}

#[cfg(feature = "z3")]
fn summarize_proofs(proofs: &[aura_nexus::ProofNote]) -> ProofSummary {
    use std::collections::BTreeMap;

    let mut by_plugin: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();

    for p in proofs {
        *by_plugin.entry(p.plugin.clone()).or_insert(0) += 1;
        *by_kind.entry(p.kind.to_string()).or_insert(0) += 1;
    }

    ProofSummary {
        total: proofs.len(),
        by_plugin: by_plugin
            .into_iter()
            .map(|(key, count)| CountBy { key, count })
            .collect(),
        by_kind: by_kind
            .into_iter()
            .map(|(key, count)| CountBy { key, count })
            .collect(),
    }
}

fn display_path(path: &Path) -> String {
    // Keep output stable and mostly relative for CI artifacts.
    // If canonicalization fails (e.g. missing file), fall back to the given path.
    let p = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    let mut s = p.to_string_lossy().to_string();
    if let Some(rest) = s.strip_prefix("\\\\?\\") {
        s = rest.to_string();
    }
    s = s.replace('\\', "/");

    if let Ok(cwd) = std::env::current_dir() {
        let mut cwd_s = cwd.to_string_lossy().to_string();
        if let Some(rest) = cwd_s.strip_prefix("\\\\?\\") {
            cwd_s = rest.to_string();
        }
        cwd_s = cwd_s.replace('\\', "/");

        let prefix = format!("{cwd_s}/");
        if s.starts_with(&prefix) {
            return s[prefix.len()..].to_string();
        }
    }

    s
}
