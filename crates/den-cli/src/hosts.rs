use crate::client::{print, Client};
use clap::Subcommand;
use den_core::*;
use reqwest::Method;
#[derive(Subcommand)]
pub enum HostCmd {
    List,
    Enroll,
}
#[derive(Subcommand)]
pub enum AccessCmd {
    Request {
        host: String,
        #[arg(value_parser=["view","control"])]
        capability: String,
        #[arg(long, conflicts_with = "standing")]
        minutes: Option<u32>,
        #[arg(long)]
        standing: bool,
    },
    Grants,
    Revoke {
        id: String,
    },
    Decide {
        id: String,
        #[arg(long)]
        allow: bool,
    },
}
#[derive(Subcommand)]
pub enum TerminalCmd {
    Open {
        host: String,
        #[arg(long = "in")]
        room: Option<String>,
    },
    Write {
        session: String,
        text: String,
    },
    Close {
        session: String,
    },
}
fn resolve(c: &Client, name: &str) -> anyhow::Result<String> {
    if name.len() == 26 {
        return Ok(crate::id(name)?.into());
    }
    let matches: Vec<Host> = c
        .get::<Vec<Host>>("/hosts")?
        .into_iter()
        .filter(|h| h.name == name)
        .collect();
    anyhow::ensure!(
        matches.len() == 1,
        "Use a unique machine name or its host ID"
    );
    Ok(matches[0].id.clone())
}
pub fn host(c: &Client, cmd: HostCmd) -> anyhow::Result<()> {
    match cmd {
        HostCmd::List => print(&c.get::<Vec<Host>>("/hosts")?)?,
        HostCmd::Enroll => {
            let v: HostEnrollment =
                c.send(Method::POST, "/hosts/enroll", &serde_json::json!({}))?;
            println!("{}", v.code);
        }
    }
    Ok(())
}
pub fn access(c: &Client, cmd: AccessCmd) -> anyhow::Result<()> {
    match cmd {
        AccessCmd::Request {
            host,
            capability,
            minutes,
            standing,
        } => print(&c.send::<Object>(
            Method::POST,
            &format!("/hosts/{}/requests", resolve(c, &host)?),
            &RequestAccess {
                capability: if capability == "control" {
                    Capability::TerminalControl
                } else {
                    Capability::TerminalView
                },
                duration_minutes: minutes,
                standing,
            },
        )?)?,
        AccessCmd::Grants => print(&c.get::<Vec<Grant>>("/grants")?)?,
        AccessCmd::Revoke { id } => {
            c.empty(Method::DELETE, &format!("/grants/{}", crate::id(&id)?))?
        }
        AccessCmd::Decide { id, allow } => print(&c.send::<AccessRequest>(
            Method::POST,
            &format!("/requests/{}/decide", crate::id(&id)?),
            &AccessDecision { allow },
        )?)?,
    }
    Ok(())
}
pub fn terminal(c: &Client, cmd: TerminalCmd) -> anyhow::Result<()> {
    match cmd {
        TerminalCmd::Open { host, room } => print(&c.send::<Object>(
            Method::POST,
            &format!("/hosts/{}/sessions", resolve(c, &host)?),
            &OpenTerminal {
                channel_id: room.map(|r| c.resolve_channel(&r)).transpose()?,
                cols: None,
                rows: None,
                thread_id: None,
                task_id: None,
                reply_to: None,
            },
        )?)?,
        TerminalCmd::Write { session, text } => {
            let response = c
                .request(
                    Method::POST,
                    &format!("/sessions/{}/write", crate::id(&session)?),
                )
                .json(&TerminalWrite { text })
                .send()?;
            anyhow::ensure!(
                response.status().is_success(),
                "Input denied ({})",
                response.status()
            );
        }
        TerminalCmd::Close { session } => c.empty(
            Method::DELETE,
            &format!("/sessions/{}", crate::id(&session)?),
        )?,
    }
    Ok(())
}
