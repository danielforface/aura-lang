#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use aura_ast::{Expr, ExprKind, Span};
use aura_nexus::{AuraPlugin, NexusContext, NexusDiagnostic, NexusFileContext, PluginCapability, PreParseResult, Z3Call, Z3CallEnv};

#[cfg(feature = "z3")]
use aura_nexus::{record_proof, ProofNote};
use miette::IntoDiagnostic;

#[derive(Clone, Debug)]
pub struct HwRegister {
    pub name: String,
    pub base: u64,
    pub size: u64,
    pub mask: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct HardwareManifest {
    pub registers: Vec<HwRegister>,
    pub manifest_path: Option<PathBuf>,
}

impl HardwareManifest {
    pub fn find_from(start: &Path) -> miette::Result<Option<PathBuf>> {
        let mut cur = if start.is_file() {
            start.parent().unwrap_or_else(|| Path::new(".")).to_path_buf()
        } else {
            start.to_path_buf()
        };

        loop {
            let candidate = cur.join("aura.toml");
            if candidate.exists() {
                return Ok(Some(candidate));
            }
            let parent = cur.parent().map(|p| p.to_path_buf());
            match parent {
                Some(p) => cur = p,
                None => return Ok(None),
            }
        }
    }

    pub fn load_from(start: &Path) -> miette::Result<Self> {
        let Some(manifest_path) = Self::find_from(start)? else {
            return Ok(Self::default());
        };

        let raw = std::fs::read_to_string(&manifest_path).into_diagnostic()?;
        let doc: toml::Value = raw
            .parse::<toml::Value>()
            .into_diagnostic()
            .map_err(|e| miette::miette!("failed to parse {}: {e}", manifest_path.display()))?;

        let mut out = HardwareManifest {
            registers: Vec::new(),
            manifest_path: Some(manifest_path),
        };

        let Some(hw) = doc.get("hardware") else {
            return Ok(out);
        };
        let regs = match hw.get("registers") {
            Some(v) => v,
            None => return Ok(out),
        };

        match regs {
            toml::Value::Array(items) => {
                for it in items {
                    if let Some(r) = parse_register(it) {
                        out.registers.push(r);
                    }
                }
            }
            toml::Value::Table(_) => {
                if let Some(r) = parse_register(regs) {
                    out.registers.push(r);
                }
            }
            _ => {}
        }

        Ok(out)
    }

    pub fn get(&self, name: &str) -> Option<&HwRegister> {
        self.registers
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(name))
    }
}

fn parse_register(v: &toml::Value) -> Option<HwRegister> {
    let t = v.as_table()?;

    let name = t.get("name")?.as_str()?.to_string();
    let base = parse_u64(t.get("base")?)?;
    let size = parse_u64(t.get("size")?)?;
    let mask = t.get("mask").and_then(parse_u64);

    Some(HwRegister {
        name,
        base,
        size,
        mask,
    })
}

fn parse_u64(v: &toml::Value) -> Option<u64> {
    match v {
        toml::Value::Integer(i) => (*i).try_into().ok(),
        toml::Value::String(s) => {
            let s = s.trim();
            if let Some(hex) = s.strip_prefix("0x") {
                u64::from_str_radix(hex, 16).ok()
            } else {
                s.parse::<u64>().ok()
            }
        }
        _ => None,
    }
}

fn is_contiguous_lsb_mask(mask: u64) -> bool {
    if mask == 0 {
        return true;
    }
    (mask & (mask + 1)) == 0
}

#[derive(Default)]
struct IotState {
    hw: HardwareManifest,
    cap_regs_by_handle: std::collections::HashMap<String, HwRegister>,
}

pub struct AuraIotPlugin;

impl AuraIotPlugin {
    pub fn new() -> Self {
        Self
    }

    fn state<'a>(nexus: &'a mut NexusContext) -> &'a mut IotState {
        if nexus.get::<IotState>().is_none() {
            nexus.insert(IotState::default());
        }
        nexus.get_mut::<IotState>().expect("inserted")
    }

    fn ensure_loaded(nexus: &mut NexusContext, span: Span) -> Result<(), NexusDiagnostic> {
        // Best-effort: load from the current file context (LSP/REPL), otherwise fall back to cwd.
        let start = nexus
            .get::<NexusFileContext>()
            .and_then(|fc| fc.manifest_path.clone())
            .unwrap_or_else(|| PathBuf::from("."));

        let st = Self::state(nexus);
        if st.hw.manifest_path.is_some() || !st.hw.registers.is_empty() {
            return Ok(());
        }

        st.hw = HardwareManifest::load_from(&start).map_err(|e| {
            NexusDiagnostic::new(
                "aura-iot",
                span,
                format!("failed to load hardware manifest: {e}"),
            )
        })?;

        Ok(())
    }

    fn eval_string_lit(expr: &Expr) -> Option<&str> {
        match &expr.kind {
            ExprKind::StringLit(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

impl AuraPlugin for AuraIotPlugin {
    fn name(&self) -> &'static str {
        "aura-iot"
    }

    fn capabilities(&self) -> &'static [PluginCapability] {
        &[PluginCapability::Z3Theories]
    }

    fn on_pre_parse(
        &self,
        _source: &str,
        _nexus: &mut NexusContext,
    ) -> Result<Option<PreParseResult>, NexusDiagnostic> {
        Ok(None)
    }

    #[cfg(feature = "z3")]
    fn on_z3_int_call<'ctx>(
        &self,
        call: &Z3Call<'_, '_>,
        env: &mut dyn Z3CallEnv<'ctx>,
    ) -> Option<Result<z3::ast::Int<'ctx>, NexusDiagnostic>> {
        match call.name {
            "hw.open" => Some(self.z3_hw_open(call, env)),
            "hw.read_u32" => Some(self.z3_hw_read_u32(call, env)),
            "hw.write_u32" => Some(self.z3_hw_write_u32(call, env)),
            _ => None,
        }
    }
}

impl AuraIotPlugin {
    #[cfg(feature = "z3")]
    fn z3_hw_open<'ctx>(
        &self,
        call: &Z3Call<'_, '_>,
        env: &mut dyn Z3CallEnv<'ctx>,
    ) -> Result<z3::ast::Int<'ctx>, NexusDiagnostic> {
        if call.args.len() != 1 {
            return Err(NexusDiagnostic::new(
                self.name(),
                call.span,
                "hw.open expects 1 arg (string literal register name)",
            ));
        }

        let reg_name = Self::eval_string_lit(call.args[0]).ok_or_else(|| {
            NexusDiagnostic::new(
                self.name(),
                call.args[0].span,
                "hw.open register name must be a string literal",
            )
        })?;

        Self::ensure_loaded(env.nexus(), call.span)?;

        let reg = {
            let st = Self::state(env.nexus());
            st.hw
                .get(reg_name)
                .cloned()
                .ok_or_else(|| {
                    NexusDiagnostic::new(
                        self.name(),
                        call.args[0].span,
                        format!("unknown hardware register '{reg_name}'"),
                    )
                })?
        };

        if let Some(mask) = reg.mask {
            if !is_contiguous_lsb_mask(mask) {
                return Err(NexusDiagnostic::new(
                    self.name(),
                    call.args[0].span,
                    format!(
                        "hardware register '{reg_name}' has unsupported mask 0x{mask:X} (must be contiguous LSB mask)"
                    ),
                ));
            }
        }

        let cap = env.fresh_int("hwcap");
        let lo = z3::ast::Int::from_u64(env.ctx(), 0);
        let hi = z3::ast::Int::from_u64(env.ctx(), 0xFFFF_FFFF);
        env.push_constraint(cap.ge(&lo));
        env.push_constraint(cap.le(&hi));

        {
            let st = Self::state(env.nexus());
            st.cap_regs_by_handle.insert(cap.to_string(), reg);
        }
        Ok(cap)
    }

    #[cfg(feature = "z3")]
    fn z3_hw_write_u32<'ctx>(
        &self,
        call: &Z3Call<'_, '_>,
        env: &mut dyn Z3CallEnv<'ctx>,
    ) -> Result<z3::ast::Int<'ctx>, NexusDiagnostic> {
        if call.args.len() != 3 {
            return Err(NexusDiagnostic::new(
                self.name(),
                call.span,
                "hw.write_u32 expects 3 args (cap, offset, value)",
            ));
        }

        let cap = env.eval_int(call.args[0])?;
        let offset = env.eval_int(call.args[1])?;
        let value = env.eval_int(call.args[2])?;

        let reg = {
            let st = Self::state(env.nexus());
            st.cap_regs_by_handle
                .get(&cap.to_string())
                .cloned()
                .ok_or_else(|| {
                    NexusDiagnostic::new(
                        self.name(),
                        call.args[0].span,
                        "hw.write_u32 requires a capability returned by hw.open",
                    )
                })?
        };

        if reg.size == 0 {
            return Err(NexusDiagnostic::new(
                self.name(),
                call.span,
                format!("hardware register '{}' has size 0 in manifest", reg.name),
            ));
        }

        let max_off = reg.size.saturating_sub(4);
        let max_off_i = z3::ast::Int::from_u64(env.ctx(), max_off);
        let ok_off = offset.le(&max_off_i);
        env.prove_implied(ok_off.not(), call.args[1].span, "MMIO write offset may be out of bounds")?;
        env.push_constraint(ok_off);

        if let Some(mask) = reg.mask {
            let mask_i = z3::ast::Int::from_u64(env.ctx(), mask);
            let ok_mask = value.le(&mask_i);
            env.prove_implied(
                ok_mask.not(),
                call.args[2].span,
                "MMIO write value may violate register mask",
            )?;
            env.push_constraint(ok_mask.clone());

            record_proof(
                env.nexus(),
                ProofNote {
                    plugin: self.name().to_string(),
                    span: call.span,
                    message: format!(
                        "Verified by Aura-IoT Plugin: '{}' register write is within bitmask limits (mask 0x{mask:X}).",
                        reg.name
                    ),
                    smt: Some(ok_mask.to_string()),
                    related: vec![
                        aura_nexus::NexusRelated {
                            span: call.args[2].span,
                            message: format!("Bitmask range: [0x0..0x{mask:X}]"),
                        },
                        aura_nexus::NexusRelated {
                            span: call.args[1].span,
                            message: format!("Offset bound: <= 0x{max_off:X}"),
                        },
                    ],
                    kind: "verified",
                    mask: Some(mask),
                    range: Some((0, mask)),
                    unsat_core: Vec::new(),
                    interpolant: None,
                },
            );
        }

        Ok(z3::ast::Int::from_u64(env.ctx(), 0))
    }

    #[cfg(feature = "z3")]
    fn z3_hw_read_u32<'ctx>(
        &self,
        call: &Z3Call<'_, '_>,
        env: &mut dyn Z3CallEnv<'ctx>,
    ) -> Result<z3::ast::Int<'ctx>, NexusDiagnostic> {
        if call.args.len() != 2 {
            return Err(NexusDiagnostic::new(
                self.name(),
                call.span,
                "hw.read_u32 expects 2 args (cap, offset)",
            ));
        }

        let cap = env.eval_int(call.args[0])?;
        let offset = env.eval_int(call.args[1])?;

        let reg = {
            let st = Self::state(env.nexus());
            st.cap_regs_by_handle
                .get(&cap.to_string())
                .cloned()
                .ok_or_else(|| {
                    NexusDiagnostic::new(
                        self.name(),
                        call.args[0].span,
                        "hw.read_u32 requires a capability returned by hw.open",
                    )
                })?
        };

        if reg.size == 0 {
            return Err(NexusDiagnostic::new(
                self.name(),
                call.span,
                format!("hardware register '{}' has size 0 in manifest", reg.name),
            ));
        }

        let max_off = reg.size.saturating_sub(4);
        let max_off_i = z3::ast::Int::from_u64(env.ctx(), max_off);
        let ok_off = offset.le(&max_off_i);
        env.prove_implied(ok_off.not(), call.args[1].span, "MMIO read offset may be out of bounds")?;
        env.push_constraint(ok_off);

        let v = env.fresh_int("mmio");
        let lo = z3::ast::Int::from_u64(env.ctx(), 0);
        let hi = z3::ast::Int::from_u64(env.ctx(), 0xFFFF_FFFF);
        env.push_constraint(v.ge(&lo));
        env.push_constraint(v.le(&hi));

        if let Some(mask) = reg.mask {
            let mask_i = z3::ast::Int::from_u64(env.ctx(), mask);
            env.push_constraint(v.le(&mask_i));
        }

        Ok(v)
    }
}
