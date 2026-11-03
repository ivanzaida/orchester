//! *Edit* and *Rewind to here* in the Orchestrator chat (ORC-333, `orchester_core::orchestrator::rewind`): a
//! conversation goes back to a point, dropping what follows it. Not while it is busy: the person stops the answer
//! first. What the agent did in the dropped messages stays done, and the conversation says so; the agent's session that
//! heard them is not resumed again, so its next turn starts a new session told the conversation as it was kept. An
//! edit is a rewind to just before the message, then the edited words sent as a new message.

use std::path::Path;

use orchester_core::{
  id::ChatId,
  orchestrator::{Dropped, RewindPoint},
  time::Timestamp,
};
use orchester_store::{ChatRewind, ProjectDb};

use crate::{
  mentions,
  messages::{self, Message},
};

/// What a rewind dropped, and the person's words of the message it went back to before, if it went back before one,
/// without the facts their mentions carried: the composer takes them back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rewound {
  pub dropped: Dropped,
  pub words: Option<String>,
}

/// Rewinds conversation `chat` of the project at `root` to `point`; nothing changes when it fails, and the message
/// says why. Blocks on SQLite.
pub fn rewind_in(root: &Path, chat: ChatId, point: RewindPoint) -> Result<Rewound, Message> {
  let rewind = ProjectDb::open(root)
    .and_then(|mut db| db.rewind_chat(chat, point, Timestamp::now()))
    .map_err(|error| messages::store_error(&error))?;
  match rewind {
    ChatRewind::Rewound { dropped, words } => Ok(Rewound {
      dropped,
      words: words.map(|words| mentions::facts::said(&words).to_owned()),
    }),
    ChatRewind::Gone => Err(Message::new("orchestrator.conversation.gone")),
    ChatRewind::Busy => Err(Message::new("orchestrator.rewind.busy")),
    ChatRewind::NotThere => Err(Message::new("orchestrator.rewind.notThere")),
  }
}
