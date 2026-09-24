use crate::client::{print, Client};
use clap::{Args, Subcommand};
use den_core::*;
use reqwest::Method;

/// Show what you (or, as an admin, someone else) are doing: "Playing Minecraft",
/// "Working on backups". Activities expire unless refreshed, so a script that
/// stops also stops claiming the activity.
#[derive(Args)]
pub struct ActivityArgs {
    /// Whose activity: a username or user ID. Admins only for anyone but yourself.
    #[arg(long, global = true, default_value = "me")]
    user: String,
    /// Which source this is, so several can stack: cli, minecraft, desktop, …
    #[arg(long, global = true, default_value = "cli")]
    slot: String,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// den activity set playing "Minecraft" --details "on the den" --ttl 90
    Set {
        /// playing, listening, watching, using or working
        kind: String,
        name: String,
        #[arg(long)]
        details: Option<String>,
        /// Seconds until it disappears without another set. Default 90, at most 86400.
        #[arg(long)]
        ttl: Option<u32>,
    },
    /// Remove the activity in this slot.
    Clear,
    /// Everyone's current activities.
    List,
}

fn kind(value: &str) -> anyhow::Result<ActivityKind> {
    Ok(match value.to_ascii_lowercase().as_str() {
        "playing" => ActivityKind::Playing,
        "listening" => ActivityKind::Listening,
        "watching" => ActivityKind::Watching,
        "using" => ActivityKind::Using,
        "working" => ActivityKind::Working,
        other => anyhow::bail!(
            "Unknown kind {other}; use playing, listening, watching, using or working"
        ),
    })
}

fn user_id(c: &Client, user: &str) -> anyhow::Result<String> {
    if user == "me" {
        return Ok("me".into());
    }
    let name = user.trim_start_matches('@');
    let users: Vec<User> = c.get("/users")?;
    users
        .iter()
        .find(|u| u.id == name || u.username.eq_ignore_ascii_case(name))
        .map(|u| u.id.clone())
        .ok_or_else(|| anyhow::anyhow!("No user {user}; see den users"))
}

pub fn run(c: &Client, args: ActivityArgs) -> anyhow::Result<()> {
    match args.command {
        Command::List => print(&c.get::<Vec<UserActivities>>("/activities")?),
        Command::Set {
            kind: k,
            name,
            details,
            ttl,
        } => {
            let path = format!(
                "/users/{}/activities/{}",
                user_id(c, &args.user)?,
                args.slot
            );
            let body = SetActivity {
                kind: kind(&k)?,
                name,
                details,
                ttl_seconds: ttl,
            };
            print(&c.send::<UserActivities>(Method::PUT, &path, &body)?)
        }
        Command::Clear => {
            let path = format!(
                "/users/{}/activities/{}",
                user_id(c, &args.user)?,
                args.slot
            );
            print(&c.send::<UserActivities>(Method::DELETE, &path, &serde_json::json!(null))?)
        }
    }
}
