//! `den` CLI. Thin wrapper over the REST API and the event stream.
//! Milestone 0: `den health` only.

use clap::{Parser, Subcommand};
use den_core::Health;

#[derive(Parser)]
#[command(name = "den", version, about = "Talk to a Den server")]
struct Cli {
    /// Server base URL
    #[arg(long, env = "DEN_URL", default_value = "http://127.0.0.1:7000")]
    url: String,
    /// API token. Each token is a full user, human or bot.
    #[arg(long, env = "DEN_TOKEN")]
    token: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Check the server is up
    Health,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Health => {
            let h: Health = ureq::get(format!("{}/health", cli.url)).call()?.body_mut().read_json()?;
            println!("{} v{}", if h.ok { "ok" } else { "down" }, h.version);
        }
    }
    Ok(())
}
