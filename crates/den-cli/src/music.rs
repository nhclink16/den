use crate::client::{print, Client};
use clap::{Args, Subcommand};
use den_core::*;
use reqwest::Method;
#[derive(Args)]
pub struct Music {
    /// Voice room name or ID. Defaults to the only voice room.
    #[arg(long, global = true)]
    room: Option<String>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Add {
        #[arg(value_name = "URL")]
        video_url: String,
    },
    Skip,
    Queue,
}
pub fn run(c: &Client, args: Music) -> anyhow::Result<()> {
    let channels: Vec<Channel> = c.get("/channels")?;
    let matches: Vec<_> = channels
        .iter()
        .filter(|r| {
            r.kind == ChannelKind::Voice
                && args
                    .room
                    .as_ref()
                    .is_none_or(|name| &r.name == name || &r.id == name)
        })
        .collect();
    anyhow::ensure!(
        matches.len() == 1,
        "Choose a voice room with --room NAME or ID"
    );
    let path = format!("/rooms/{}/music", matches[0].id);
    let queue: MusicQueue = match args.command {
        Command::Add { video_url } => c.send(
            Method::POST,
            &format!("{path}/queue"),
            &AddMusic { url: video_url },
        )?,
        Command::Skip => c.send(
            Method::POST,
            &format!("{path}/skip"),
            &serde_json::json!({}),
        )?,
        Command::Queue => c.get(&path)?,
    };
    print(&queue)
}
