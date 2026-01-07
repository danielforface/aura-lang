#![forbid(unsafe_code)]

use std::io::{self, BufRead, Read};

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use miette::IntoDiagnostic;

use aura_interpret::{Avm, AvmConfig, AvmValue};

#[derive(Parser, Debug)]
#[command(name = "aura-interpret", version, about = "Aura Virtual Machine (AVM) interpreter")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Execute a source file (or stdin)
    Run {
        /// Path to .aura file; if omitted, reads from stdin
        #[arg(long)]
        file: Option<std::path::PathBuf>,

        /// Disable Z3 safety gate
        #[arg(long, default_value_t = false)]
        no_z3: bool,

        /// Print machine-readable JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Start a REPL over stdin/stdout using JSON lines
    Repl {
        /// Disable Z3 safety gate
        #[arg(long, default_value_t = false)]
        no_z3: bool,
    },
}

#[derive(Debug, Serialize)]
struct JsonOut {
    verified: bool,
    value: String,
    stdout: String,
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Run { file, no_z3, json } => {
            let src = if let Some(p) = file {
                std::fs::read_to_string(p).into_diagnostic()?
            } else {
                let mut buf = String::new();
                io::stdin().read_to_string(&mut buf).into_diagnostic()?;
                buf
            };

            let mut avm = Avm::new(AvmConfig {
                enable_z3_gate: !no_z3,
                ..Default::default()
            });

            let out = avm.exec_source(&src)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&JsonOut {
                        verified: out.verified,
                        value: format_value(&out.value),
                        stdout: out.stdout,
                    })
                    .into_diagnostic()?
                );
            } else {
                if !out.stdout.is_empty() {
                    print!("{}", out.stdout);
                }
                println!("{}", format_value(&out.value));
            }

            Ok(())
        }
        Cmd::Repl { no_z3 } => repl_main(!no_z3),
    }
}

#[derive(Debug, Deserialize)]
struct ReplIn {
    source: String,
}

fn repl_main(enable_z3: bool) -> miette::Result<()> {
    let mut avm = Avm::new(AvmConfig {
        enable_z3_gate: enable_z3,
        ..Default::default()
    });

    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut line = String::new();
    loop {
        line.clear();
        if stdin.read_line(&mut line).into_diagnostic()? == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let inp: ReplIn = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{{\"verified\":false,\"value\":\"<parse error>\",\"stdout\":\"{}\"}}", escape_json(&format!("{e}")));
                continue;
            }
        };

        match avm.exec_source(&inp.source) {
            Ok(out) => {
                let msg = JsonOut {
                    verified: out.verified,
                    value: format_value(&out.value),
                    stdout: out.stdout,
                };
                println!("{}", serde_json::to_string(&msg).into_diagnostic()?);
            }
            Err(e) => {
                let msg = JsonOut {
                    verified: false,
                    value: "<error>".to_string(),
                    stdout: e.to_string(),
                };
                println!("{}", serde_json::to_string(&msg).into_diagnostic()?);
            }
        }
    }

    Ok(())
}

fn format_value(v: &AvmValue) -> String {
    match v {
        AvmValue::Int(i) => i.to_string(),
        AvmValue::Bool(b) => b.to_string(),
        AvmValue::Str(s) => s.clone(),
        AvmValue::Style(map) => {
            let mut out = String::from("Style{");
            for (i, (k, vv)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(k);
                out.push(':');
                out.push_str(&format_value(vv));
            }
            out.push('}');
            out
        }
        AvmValue::Ui(node) => aura_nexus::format_ui_tree(node),
        AvmValue::Unit => "()".to_string(),
    }
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
