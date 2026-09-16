mod canvas;
mod client;
mod hosts;
mod stream;
use anyhow::Context;
use clap::{Parser, Subcommand};
use client::{print, Client};
use den_core::*;
use reqwest::Method;
use std::collections::BTreeMap;
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
        /// Page size, 1-200. With --all this is how many messages each fetch returns.
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Fetch every matching page and print one ascending, duplicate-free array.
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
fn password(stdin: bool) -> anyhow::Result<String> {    if let Ok(value) = std::env::var("DEN_PASSWORD") {
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
            anyhow::ensure!(
                (1..=200).contains(&limit),
                "Limit must be 1-200"
            );
            anyhow::ensure!(
                !(before.is_some() && after.is_some()),
                "Use --before or --after, not both"
            );
            // Resolve names/@users exactly once, before any paging.
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
                // before or no cursor: newest page first, walking backwards
                // towards older messages. after: oldest page first, forwards.
                let use_before = match (&before, &after) {
                    (Some(v), _) => {
                        id(v)?;
                        true
                    }
                    (None, Some(v)) => {
                        id(v)?;
                        false
                    }
                    (None, None) => true,
                };
                // An empty cursor means no bound: start at the newest page.
                let mut cursor = String::new();
                if let Some(v) = before.as_ref().or(after.as_ref()) {
                    cursor = v.clone();
                }
                let mut merged: BTreeMap<String, Message> = BTreeMap::new();
                loop {
                    let mut path = format!("/channels/{cid}/messages?limit={limit}");
                    if !cursor.is_empty() {
                        let key = if use_before { "before" } else { "after" };
                        path.push_str(&format!("&{key}={cursor}"));
                    }
                    let page: Vec<Message> = c.get(&path)?;
                    // A full page whose first id we already stored means the
                    // cursor made no progress; fail instead of looping forever.
                    if page.len() == limit as usize
                        && page.first().is_some_and(|m| merged.contains_key(&m.id))
                    {
                        anyhow::bail!("Message listing did not advance; aborting --all");
                    }
                    if page.is_empty() {
                        break;
                    }
                    let page_len = page.len();
                    for m in page {
                        merged.insert(m.id.clone(), m);
                    }
                    // A short page means we have reached the end of the range.
                    if page_len < limit as usize {
                        break;
                    }
                    // Continue from the far end of this page, exclusive.
                    let next = if use_before {
                        // Pages come back ascending; the oldest message of a
                        // backwards page is its first id, which is the next
                        // exclusive lower bound.
                        merged
                            .values()
                            .filter(|m| cursor.is_empty() || m.id < cursor)
                            .map(|m| m.id.clone())
                            .next()
                    } else {
                        merged
                            .values()
                            .filter(|m| m.id > cursor)
                            .map(|m| m.id.clone())
                            .next_back()
                    };
                    match next {
                        Some(next) => cursor = next,
                        None => break,
                    }
                }
                let merged: Vec<Message> = merged.into_values().collect();
                print(&merged)?;
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
