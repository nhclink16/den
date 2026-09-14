use crate::{
    client::{print, Client},
    id,
};
use clap::Subcommand;
use den_core::*;
use reqwest::Method;
use std::io::Read;

#[derive(Subcommand)]
pub enum Cmd {
    Create {
        channel: String,
        name: String,
    },
    Get {
        id: String,
    },
    Patch {
        id: String,
        #[arg(long)]
        file: String,
    },
    Rename {
        id: String,
        name: String,
    },
}
pub fn run(c: &Client, cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::Create { channel, name } => print(&c.send::<Object>(
            Method::POST,
            &format!("/channels/{}/objects", c.resolve_channel(&channel)?),
            &CreateObject {
                kind: "canvas".into(),
                name,
                state: Default::default(),
            },
        )?)?,
        Cmd::Get { id: oid } => print(&c.get::<Object>(&format!("/objects/{}", id(&oid)?))?.state)?,
        Cmd::Patch { id: oid, file } => {
            let text = if file == "-" {
                let mut s = String::new();
                std::io::stdin().read_to_string(&mut s)?;
                s
            } else {
                std::fs::read_to_string(file)?
            };
            let patch: ObjectPatch = serde_json::from_str(&text)?;
            print(&c.send::<ObjectVersion>(
                Method::POST,
                &format!("/objects/{}/patch", id(&oid)?),
                &patch,
            )?)?;
        }
        Cmd::Rename { id: oid, name } => print(&c.send::<ObjectSummary>(
            Method::PATCH,
            &format!("/objects/{}", id(&oid)?),
            &UpdateObject {
                name: Some(name),
                thumbnail_upload_id: None,
            },
        )?)?,
    }
    Ok(())
}
