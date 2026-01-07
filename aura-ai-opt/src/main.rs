#![forbid(unsafe_code)]

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "aura-ai-opt", version, about = "Aura AI IR Optimizer (metadata injection)")]
struct Cli {
    /// Input .ll file
    input: PathBuf,

    /// Output .ll file
    #[arg(long)]
    output: PathBuf,
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    aura_ai_opt::optimize_ll_file(&cli.input, &cli.output)?;
    println!("wrote {}", cli.output.display());
    Ok(())
}
