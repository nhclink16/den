use crate::{
    client::{print, Client},
    id,
};
use clap::Subcommand;
use den_core::*;
use reqwest::Method;

/// Conversation context a runner supplies with the work it posts. It is attached
/// only to the commands that create something; reads, edits and machine access
/// never carry it, so a job's ID cannot leak into an unrelated request.
#[derive(clap::Args, Clone, Default)]
pub struct Context {
    /// Job identity, stable for the life of one job, scoped to this author and
    /// room. An explicit value overrides DEN_TASK_ID. It is never derived from a
    /// token, process, room or elapsed time.
    #[arg(long, env = "DEN_TASK_ID")]
    pub task: Option<String>,
    /// Explicit conversation. The server rejects it if it disagrees with the job.
    #[arg(long)]
    pub thread: Option<String>,
    /// Message this is a reply to.
    #[arg(long)]
    pub reply_to: Option<String>,
}
impl Context {
    /// Validated (thread, task, reply_to) in the order the shared types take them.
    /// A task ID is opaque, so only the two ULIDs are checked here; the server has
    /// the final say on whether they agree with each other.
    pub fn parts(self) -> anyhow::Result<(Option<String>, Option<String>, Option<String>)> {
        let check = |v: Option<String>| -> anyhow::Result<Option<String>> {
            Ok(match v {
                Some(v) => Some(id(&v)?.to_owned()),
                None => None,
            })
        };
        // An exactly empty task means no job. clap reports a set-but-empty
        // DEN_TASK_ID as Some(""), so without this `DEN_TASK_ID= den send ...`
        // would reach the server as a zero-length task and return a confusing 400.
        // Only the empty string is treated this way; whitespace still goes to the
        // server, which owns task validation.
        let task = self.task.filter(|t| !t.is_empty());
        Ok((check(self.thread)?, task, check(self.reply_to)?))
    }
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Conversations in a channel. Without --resolved this lists every one.
    List {
        /// Channel ULID, text channel name (general or #general), or @username for a DM.
        channel: String,
        /// false lists the open strip, true lists resolved history.
        #[arg(long)]
        resolved: Option<bool>,
        /// Only conversations contributing unread activity to this channel's total.
        #[arg(long)]
        unread_only: bool,
        #[arg(long)]
        before: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
    /// One conversation and your own position in it.
    Get {
        thread: String,
    },
    /// Fetch replies. Like `den read`, this never advances a read position.
    Read {
        thread: String,
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        after: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
    Rename {
        thread: String,
        title: String,
    },
    /// End the conversation. A later post with its task ID is a conflict rather
    /// than a silent reopen. It stops new content, not a running terminal.
    Resolve {
        thread: String,
    },
    Reopen {
        thread: String,
    },
    /// Join from now: your position moves to the conversation's current tail, so
    /// earlier replies stop counting even if you were already following.
    Follow {
        thread: String,
    },
    /// Stop following. Your read position is kept, and this alone does not silence
    /// the conversation: a DM you belong to, or a room you subscribe to, still
    /// counts its replies.
    Unfollow {
        thread: String,
    },
    /// Advance your position in this conversation only, through that message.
    MarkRead {
        thread: String,
        message: String,
    },
}

pub fn run(c: &Client, cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::List {
            channel,
            resolved,
            unread_only,
            before,
            limit,
        } => {
            let mut path = format!(
                "/channels/{}/threads?limit={limit}",
                c.resolve_channel(&channel)?
            );
            if let Some(v) = resolved {
                path.push_str(&format!("&resolved={v}"));
            }
            if unread_only {
                path.push_str("&unread_only=true");
            }
            if let Some(v) = before {
                path.push_str(&format!("&before={}", id(&v)?));
            }
            print(&c.get::<Vec<ThreadView>>(&path)?)?
        }
        Cmd::Get { thread } => print(&c.get::<ThreadView>(&format!("/threads/{}", id(&thread)?))?)?,
        Cmd::Read {
            thread,
            before,
            after,
            limit,
        } => {
            let mut path = format!("/threads/{}/messages?limit={limit}", id(&thread)?);
            if let Some(v) = before {
                path.push_str(&format!("&before={}", id(&v)?));
            }
            if let Some(v) = after {
                path.push_str(&format!("&after={}", id(&v)?));
            }
            print(&c.get::<Vec<Message>>(&path)?)?
        }
        Cmd::Rename { thread, title } => print(&c.send::<ThreadSummary>(
            Method::PATCH,
            &format!("/threads/{}", id(&thread)?),
            &UpdateThread {
                title: Some(title),
                resolved: None,
            },
        )?)?,
        Cmd::Resolve { thread } => print(&c.send::<ThreadSummary>(
            Method::PATCH,
            &format!("/threads/{}", id(&thread)?),
            &UpdateThread {
                title: None,
                resolved: Some(true),
            },
        )?)?,
        Cmd::Reopen { thread } => print(&c.send::<ThreadSummary>(
            Method::PATCH,
            &format!("/threads/{}", id(&thread)?),
            &UpdateThread {
                title: None,
                resolved: Some(false),
            },
        )?)?,
        Cmd::Follow { thread } => print(&c.send::<ThreadReadState>(
            Method::PUT,
            &format!("/threads/{}/follow", id(&thread)?),
            &FollowThread { following: true },
        )?)?,
        Cmd::Unfollow { thread } => print(&c.send::<ThreadReadState>(
            Method::PUT,
            &format!("/threads/{}/follow", id(&thread)?),
            &FollowThread { following: false },
        )?)?,
        Cmd::MarkRead { thread, message } => print(&c.send::<ThreadReadState>(
            Method::PUT,
            &format!("/threads/{}/read", id(&thread)?),
            &MarkThreadRead {
                message_id: id(&message)?.into(),
            },
        )?)?,
    }
    Ok(())
}
