//! *Fork from here* in the Orchestrator chat (ORC-334, `orchester_core::orchestrator::fork`): a new conversation holds
//! copies of a conversation's messages up to a point, and the original stays as it was. Unlike a rewind it drops
//! nothing, so it needs no question and may be made while the Orchestrator answers: it takes what was kept so far. Its
//! agent starts a new session, told the copied conversation; what the original did after the point stays there.

use std::path::Path;

use orchester_core::{id::ChatId, orchestrator::RewindPoint, time::Timestamp};
use orchester_store::{ChatFork, ProjectDb};

use crate::{
  mentions,
  messages::{self, Message},
};

/// The fork kept, and the person's words of the message it stopped before, if it stopped before one, without the facts
/// their mentions carried: the fork's composer takes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Forked {
  pub chat: ChatId,
  pub words: Option<String>,
}

/// Forks conversation `from` of the project at `root` at `point` into a new conversation titled `title`; nothing is
/// kept when it fails, and the message says why. Blocks on SQLite.
pub fn fork_in(root: &Path, (from, point): (ChatId, RewindPoint), title: &str) -> Result<Forked, Message> {
  let fork = ProjectDb::open(root)
    .and_then(|mut db| db.fork_chat((from, point), (ChatId::generate(), title), Timestamp::now()))
    .map_err(|error| messages::store_error(&error))?;
  match fork {
    ChatFork::Forked { chat, words } => Ok(Forked {
      chat,
      words: words.map(|words| mentions::facts::said(&words).to_owned()),
    }),
    ChatFork::Gone => Err(Message::new("orchestrator.conversation.gone")),
    ChatFork::NotThere => Err(Message::new("orchestrator.fork.notThere")),
  }
}
