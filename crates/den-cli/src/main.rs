mod canvas;
mod client;
mod hosts;
mod stream;
use anyhow::Context;
use clap::{Parser, Subcommand};
use client::{print, Client};
use den_core::*;
use reqwest::Method;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "den",
    version,
    about = "Talk to a Den server; command results are JSON"
)]
struct Cli {
    #[arg(
        long,
        env = "DEN_URL",
        default_value = "http://127.0.0.1:7000",
        global = true
    )]
    url: String,
    #[arg(long, env = "DEN_TOKEN", global = true, hide_env_values = true)]
    token: Option<String>,
    #[arg(long, env = "DEN_CONFIG", global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}
#[derive(Subcommand)]
enum Cmd {
    Health,
    #[command(subcommand)]
    Host(hosts::HostCmd),
    #[command(subcommand)]
    Access(hosts::AccessCmd),
    #[command(subcommand)]
    Terminal(hosts::TerminalCmd),
    #[command(subcommand)]
    Canvas(canvas::Cmd),
    /// Claim a new local instance using its one-time key file.
    Init {
        username: String,
        #[arg(long, default_value = "data/bootstrap.key")]
        bootstrap_file: PathBuf,
        #[arg(long)]
        password_stdin: bool,
    },
    Login {
        username: String,
        #[arg(long)]
        password_stdin: bool,
    },
    Register {
        username: String,
        #[arg(long, env = "DEN_INVITE", hide_env_values = true)]
        invite: String,
        #[arg(long)]
        password_stdin: bool,
    },
    Logout,
    Me,
    Users,
    Channels,
    #[command(subcommand)]
    Channel(ChannelCmd),
    #[command(subcommand)]
    Category(CategoryCmd),
    #[command(subcommand)]
    Invite(InviteCmd),
    /// Create or reopen a DM with the exact member set, including yourself.
    Dm {
        /// User ULIDs or @usernames. Your own user is included automatically.
        members: Vec<String>,
    },
    Send {
        /// Channel ULID, text channel name (general or #general), or @username for a DM.
        channel: String,
        text: String,
        #[arg(long)]
        reply_to: Option<String>,
        #[arg(long = "upload")]
        uploads: Vec<String>,
    },
    Read {
        /// Channel ULID, text channel name (general or #general), or @username for a DM.
        channel: String,
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        after: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Fetch every matching page and print one ascending JSON array.
        #[arg(long)]
        all: bool,
    },
    Edit {
        message: String,
        text: String,
    },
    Delete {
        message: String,
    },
    /// Print events as JSON lines; reconnects report gaps requiring a fresh read.
    Tail {
        /// Channel ULID, text channel name (general or #general), or @username for a DM.
        channel: Option<String>,
    },
    /// Upload a file; use the printed ID with --resume after interruption.
    Upload {
        /// Channel ULID, text channel name (general or #general), or @username for a DM.
        channel: String,
        file: PathBuf,
        #[arg(long)]
        resume: Option<String>,
        #[arg(long)]
        content_type: Option<String>,
    },
    #[command(subcommand)]
    Token(TokenCmd),
    #[command(subcommand)]
    Bot(BotCmd),
}
#[derive(Subcommand)]
enum ChannelCmd {
    Create {
        name: String,
        #[arg(long)]
        category: Option<String>,
        #[arg(long, default_value_t = 0)]
        position: i64,
    },
    Update {
        /// Channel ULID, text channel name (general or #general), or @username for a DM.
        #[arg(value_name = "CHANNEL")]
        id: String,
        name: String,
        #[arg(long)]
        category: Option<String>,
        #[arg(long, default_value_t = 0)]
        position: i64,
    },
    Delete {
        /// Channel ULID, text channel name (general or #general), or @username for a DM.
        #[arg(value_name = "CHANNEL")]
        id: String,
    },
}
#[derive(Subcommand)]
enum CategoryCmd {
    List,
    Create {
        name: String,
        #[arg(long, default_value_t = 0)]
        position: i64,
    },
    Update {
        id: String,
        name: String,
        #[arg(long, default_value_t = 0)]
        position: i64,
    },
    Delete {
        id: String,
    },
}
#[derive(Subcommand)]
enum InviteCmd {
    Create {
        #[arg(long, default_value_t = 1)]
        uses: u32,
        #[arg(long, default_value_t = 24)]
        hours: u32,
    },
    Revoke {
        id: String,
    },
}
#[derive(Subcommand)]
enum TokenCmd {
    Create {
        name: String,
        #[arg(long)]
        user: Option<String>,
    },
    List,
    Revoke {
        id: String,
    },
}
#[derive(Subcommand)]
enum BotCmd {
    Create {
        username: String,
        #[arg(long)]
        display_name: Option<String>,
    },
}
fn password(stdin: bool) -> anyhow::Result<String> {
    if let Ok(value) = std::env::var("DEN_PASSWORD") {
        return Ok(value);
    }
    anyhow::ensure!(stdin, "Set DEN_PASSWORD or pass --password-stdin");
    let mut value = String::new();
    std::io::stdin().read_line(&mut value)?;
    Ok(value.trim_end_matches(['\r', '\n']).to_string())
}
fn id(value: &str) -> anyhow::Result<&str> {
    anyhow::ensure!(
        value.len() == 26 && value.bytes().all(|b| b.is_ascii_alphanumeric()),
        "Expected a ULID"
    );
    Ok(value)
}
fn check_read_args(before: Option<&str>, after: Option<&str>, limit: u32) -> anyhow::Result<()> {
    if before.is_some() && after.is_some() {
        anyhow::bail!("Use before or after, not both");
    }
    if !(1..=200).contains(&limit) {
        anyhow::bail!("Limit must be 1-200");
    }
    if let Some(v) = before {
        id(v)?;
    }
    if let Some(v) = after {
        id(v)?;
    }
    Ok(())
}
fn sort_messages(mut out: Vec<Message>) -> Vec<Message> {
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.id == b.id);
    out
}
fn read_all(
    c: &Client,
    channel_id: &str,
    before: Option<String>,
    after: Option<String>,
    limit: u32,
) -> anyhow::Result<Vec<Message>> {
    use std::collections::HashSet;
    let mut out: Vec<Message> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    if let Some(start) = after {
        let mut cursor = start;
        loop {
            let path = format!("/channels/{channel_id}/messages?limit={limit}&after={cursor}");
            let page: Vec<Message> = c.get(&path)?;
            if page.is_empty() {
                break;
            }
            let len = page.len();
            // Server pages are ascending; every id must be past the cursor.
            if page.first().is_some_and(|m| m.id <= cursor) {
                anyhow::bail!("Server returned no progress; aborting pagination");
            }
            let next = page.last().expect("nonempty").id.clone();
            if next == cursor {
                anyhow::bail!("Server returned no progress; aborting pagination");
            }
            for m in page {
                if seen.insert(m.id.clone()) {
                    out.push(m);
                }
            }
            if len < limit as usize {
                break;
            }
            // Full page with no cursor advance would loop forever.
            if next == cursor {
                anyhow::bail!("Server returned no progress; aborting pagination");
            }
            cursor = next;
        }
    } else {
        let mut cursor: Option<String> = before;
        loop {
            let mut path = format!("/channels/{channel_id}/messages?limit={limit}");
            if let Some(v) = &cursor {
                path.push_str(&format!("&before={v}"));
            }
            let page: Vec<Message> = c.get(&path)?;
            if page.is_empty() {
                break;
            }
            let len = page.len();
            let first = page.first().expect("nonempty").id.clone();
            let last = page.last().expect("nonempty").id.clone();
            if let Some(cur) = &cursor {
                // Newest-matching page must sit strictly below the cursor.
                if last >= *cur {
                    anyhow::bail!("Server returned no progress; aborting pagination");
                }
                if first == *cur {
                    anyhow::bail!("Server returned no progress; aborting pagination");
                }
            } else if len == limit as usize && seen.contains(&first) && seen.contains(&last) {
                anyhow::bail!("Server returned no progress; aborting pagination");
            }
            // A full page that does not move the cursor would loop forever.
            if cursor.as_ref() == Some(&first) {
                anyhow::bail!("Server returned no progress; aborting pagination");
            }
            for m in page {
                if seen.insert(m.id.clone()) {
                    out.push(m);
                }
            }
            if len < limit as usize {
                break;
            }
            cursor = Some(first);
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.id == b.id);
    Ok(sort_messages(out))
}
fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let mut c = Client::new(args.url, args.token, args.config)?;
    match args.cmd {
        Cmd::Canvas(cmd) => canvas::run(&c, cmd)?,
        Cmd::Host(cmd) => hosts::host(&c, cmd)?,
        Cmd::Access(cmd) => hosts::access(&c, cmd)?,
        Cmd::Terminal(cmd) => hosts::terminal(&c, cmd)?,
        Cmd::Health => print(&c.get::<Health>("/health")?)?,
        Cmd::Init {
            username,
            bootstrap_file,
            password_stdin,
        } => {
            let session: Session = c.send(
                Method::POST,
                "/auth/init",
                &Bootstrap {
                    username,
                    password: password(password_stdin)?,
                    bootstrap_token: std::fs::read_to_string(bootstrap_file)
                        .context("Read the bootstrap key on the server machine")?,
                },
            )?;
            print(&session.user)?;
            c.save(session)?;
        }
        Cmd::Login {
            username,
            password_stdin,
        } => {
            let session: Session = c.send(
                Method::POST,
                "/auth/login",
                &Login {
                    username,
                    password: password(password_stdin)?,
                },
            )?;
            print(&session.user)?;
            c.save(session)?;
        }
        Cmd::Register {
            username,
            invite,
            password_stdin,
        } => {
            let session: Session = c.send(
                Method::POST,
                "/auth/register",
                &Register {
                    username,
                    password: password(password_stdin)?,
                    invite,
                },
            )?;
            print(&session.user)?;
            c.save(session)?;
        }
        Cmd::Logout => {
            c.empty(Method::POST, "/auth/logout")?;
            c.forget()?;
        }
        Cmd::Me => print(&c.get::<User>("/users/me")?)?,
        Cmd::Users => print(&c.get::<Vec<User>>("/users")?)?,
        Cmd::Channels => print(&c.get::<Vec<Channel>>("/channels")?)?,
        Cmd::Channel(v) => match v {
            ChannelCmd::Create {
                name,
                category,
                position,
            } => print(&c.send::<Channel>(
                Method::POST,
                "/channels",
                &SaveChannel {
                    name,
                    category_id: category,
                    position,
                },
            )?)?,
            ChannelCmd::Update {
                id: cid,
                name,
                category,
                position,
            } => print(&c.send::<Channel>(
                Method::PUT,
                &format!("/channels/{}", c.resolve_channel(&cid)?),
                &SaveChannel {
                    name,
                    category_id: category,
                    position,
                },
            )?)?,
            ChannelCmd::Delete { id: cid } => c.empty(
                Method::DELETE,
                &format!("/channels/{}", c.resolve_channel(&cid)?),
            )?,
        },
        Cmd::Category(v) => match v {
            CategoryCmd::List => print(&c.get::<Vec<Category>>("/categories")?)?,
            CategoryCmd::Create { name, position } => print(&c.send::<Category>(
                Method::POST,
                "/categories",
                &SaveCategory { name, position },
            )?)?,
            CategoryCmd::Update {
                id: cid,
                name,
                position,
            } => print(&c.send::<Category>(
                Method::PUT,
                &format!("/categories/{}", id(&cid)?),
                &SaveCategory { name, position },
            )?)?,
            CategoryCmd::Delete { id: cid } => {
                c.empty(Method::DELETE, &format!("/categories/{}", id(&cid)?))?
            }
        },
        Cmd::Invite(v) => match v {
            InviteCmd::Create { uses, hours } => print(&c.send::<Invite>(
                Method::POST,
                "/invites",
                &CreateInvite {
                    uses,
                    expires_in_hours: hours,
                },
            )?)?,
            InviteCmd::Revoke { id: cid } => {
                c.empty(Method::DELETE, &format!("/invites/{}", id(&cid)?))?
            }
        },
        Cmd::Dm { members } => print(&c.dm(&members)?)?,
        Cmd::Send {
            channel,
            text,
            reply_to,
            uploads,
        } => print(&c.send::<Message>(
            Method::POST,
            &format!("/channels/{}/messages", c.resolve_channel(&channel)?),
            &CreateMessage {
                content: text,
                reply_to,
                upload_ids: uploads,
            },
        )?)?,
        Cmd::Read {
            channel,
            before,
            after,
            limit,
            all,
        } => {
            check_read_args(before.as_deref(), after.as_deref(), limit)?;
            let cid = c.resolve_channel(&channel)?;
            if !all {
                let mut path = format!("/channels/{cid}/messages?limit={limit}");
                if let Some(v) = before {
                    path.push_str(&format!("&before={}", id(&v)?));
                }
                if let Some(v) = after {
                    path.push_str(&format!("&after={}", id(&v)?));
                }
                print(&c.get::<Vec<Message>>(&path)?)?;
            } else {
                print(&read_all(&c, &cid, before, after, limit)?)?;
            }
        }
        Cmd::Edit { message, text } => print(&c.send::<Message>(
            Method::PATCH,
            &format!("/messages/{}", id(&message)?),
            &EditMessage { content: text },
        )?)?,
        Cmd::Delete { message } => {
            c.empty(Method::DELETE, &format!("/messages/{}", id(&message)?))?
        }
        Cmd::Tail { channel } => {
            let channel = channel
                .as_deref()
                .map(|v| c.resolve_channel(v))
                .transpose()?;
            stream::tail(&c, channel)?;
        }
        Cmd::Upload {
            channel,
            file,
            resume,
            content_type,
        } => stream::upload(
            &c,
            &c.resolve_channel(&channel)?,
            &file,
            resume,
            content_type,
        )?,
        Cmd::Token(v) => match v {
            TokenCmd::Create { name, user } => print(&c.send::<TokenSecret>(
                Method::POST,
                "/tokens",
                &CreateToken {
                    name,
                    user_id: user,
                },
            )?)?,
            TokenCmd::List => print(&c.get::<Vec<Token>>("/tokens")?)?,
            TokenCmd::Revoke { id: tid } => {
                c.empty(Method::DELETE, &format!("/tokens/{}", id(&tid)?))?
            }
        },
        Cmd::Bot(BotCmd::Create {
            username,
            display_name,
        }) => print(&c.send::<BotCreated>(
            Method::POST,
            "/bots",
            &CreateBot {
                display_name: display_name.unwrap_or_else(|| username.clone()),
                username,
            },
        )?)?,
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn msg(id: &str) -> Message {
        Message {
            id: id.to_string(),
            channel_id: "c".into(),
            author_id: "a".into(),
            content: "hi".into(),
            reply_to: None,
            created_at: "t".into(),
            edited_at: None,
            attachments: vec![],
            objects: vec![],
            reactions: vec![],
            mention_ids: vec![],
        }
    }
    #[test]
    fn read_args_reject_both_cursors_and_bad_limits() {
        let good = "01JAAAAAAAAAAAAAAAAAAAAAAAAA"[..26].to_string();
        assert!(check_read_args(Some(&good), Some(&good), 50).is_err());
        assert!(check_read_args(None, None, 0).is_err());
        assert!(check_read_args(None, None, 201).is_err());
        assert!(check_read_args(None, None, 1).is_ok());
        assert!(check_read_args(None, None, 200).is_ok());
        assert!(check_read_args(Some("short"), None, 50).is_err());
    }
    #[test]
    fn sort_messages_orders_ascending_and_dedups() {
        let out = sort_messages(vec![msg("02"), msg("01"), msg("02"), msg("03")]);
        let ids: Vec<_> = out.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["01", "02", "03"]);
    }
}
