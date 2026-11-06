//! The Orchestrator chat's conversations as the screen reads them: History's list and one conversation's entries,
//! with the files the person attached to their messages, the todo list each turn's agent keeps, which of its waits
//! still stand, what became of the runs whose ends no longer do, and how full its session's context is; and what a
//! person changes of a conversation in History: its title and its pin (ORC-328), its archive and its deletion
//! (ORC-329).

use std::{
  cmp::Reverse,
  collections::{BTreeMap, HashMap, HashSet},
  path::Path,
};

use orchester_core::{
  attachment::Attachment,
  connection::{context_use::ContextUse, stream::TodoList},
  id::{ChatId, ChatMessageId},
  orchestrator::{BlankTitle, Chat, ChatEntry, ChatTitle, EntryKind, TurnState},
  run::RunStatus,
  time::Timestamp,
};
use orchester_store::{ChatDeletion, ChatListing, ProjectDb};

use crate::messages::{self, Message};

/// The project's conversations, archived ones too, the one with the latest entry first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
  pub chats: Vec<ChatListing>,
}

impl History {
  /// The conversations History lists: those not archived, the one with the latest entry first.
  pub fn listed(&self) -> impl Iterator<Item = &ChatListing> {
    self.chats.iter().filter(|listing| listing.archived.is_none())
  }

  /// The archived conversations, the one archived last first.
  pub fn archived(&self) -> Vec<&ChatListing> {
    let mut archived: Vec<&ChatListing> = self.chats.iter().filter(|listing| listing.archived.is_some()).collect();
    archived.sort_by_key(|listing| Reverse(listing.archived));
    archived
  }
}

/// Every conversation of the project at `root`. Blocks on SQLite.
pub fn history_in(root: &Path) -> Result<History, Message> {
  ProjectDb::open(root)
    .and_then(|db| db.chats())
    .map(|chats| History { chats })
    .map_err(|error| messages::store_error(&error))
}

/// Titles conversation `chat` of the project at `root` `title`, as a person named it in History; it keeps that title
/// from then on. A blank title is refused. Blocks on SQLite.
pub fn rename_in(root: &Path, chat: ChatId, title: &str) -> Result<(), Message> {
  let title = ChatTitle::new(title).map_err(|BlankTitle| Message::new("orchestrator.conversation.blankTitle"))?;
  ProjectDb::open(root)
    .and_then(|mut db| db.rename_chat(chat, &title))
    .map_err(|error| messages::store_error(&error))
    .and_then(kept)
}

/// Pins conversation `chat` of the project at `root` to History's top as of `at`, which lists the latest pin first, or
/// unpins it (`None`). Blocks on SQLite.
pub fn pin_in(root: &Path, chat: ChatId, at: Option<Timestamp>) -> Result<(), Message> {
  ProjectDb::open(root)
    .and_then(|mut db| db.pin_chat(chat, at))
    .map_err(|error| messages::store_error(&error))
    .and_then(kept)
}

/// Archives conversation `chat` of the project at `root` as of `at`, which lists it under History's Archived and
/// unpins it, or unarchives it (`None`), which lists it under its day again. Blocks on SQLite.
pub fn archive_in(root: &Path, chat: ChatId, at: Option<Timestamp>) -> Result<(), Message> {
  ProjectDb::open(root)
    .and_then(|mut db| db.archive_chat(chat, at))
    .map_err(|error| messages::store_error(&error))
    .and_then(kept)
}

/// Deletes conversation `chat` of the project at `root` with its messages and the files attached to them, from the
/// project's own database and folder; refused while the Orchestrator answers in it, compacts its session or a message
/// waits there for its turn. Blocks on SQLite.
pub fn delete_in(root: &Path, chat: ChatId) -> Result<(), Message> {
  let deleted = ProjectDb::open(root)
    .and_then(|mut db| db.delete_chat(chat))
    .map_err(|error| messages::store_error(&error))?;
  match deleted {
    ChatDeletion::Deleted => Ok(()),
    ChatDeletion::Gone => Err(Message::new("orchestrator.conversation.gone")),
    ChatDeletion::Busy => Err(Message::new("orchestrator.conversation.busy")),
  }
}

/// A change of a conversation the project no longer keeps did not happen.
fn kept(changed: bool) -> Result<(), Message> {
  if changed {
    Ok(())
  } else {
    Err(Message::new("orchestrator.conversation.gone"))
  }
}

/// How full the context of the session conversation `chat`'s turns continue is, read alone: the Orchestrator's context
/// meter reads it again as turns arrive, without the conversation's entries. `None` while no turn's CLI reported it.
/// Blocks on SQLite.
pub fn context_in(root: &Path, chat: ChatId) -> Result<Option<ContextUse>, Message> {
  ProjectDb::open(root)
    .and_then(|db| db.chat_context(chat))
    .map_err(|error| messages::store_error(&error))
}

/// One conversation and its entries in the order they were kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatView {
  pub chat: Chat,
  pub entries: Vec<ChatEntry>,
  /// The files attached to the person's messages, by message, in the order they were attached.
  pub attachments: BTreeMap<ChatMessageId, Vec<Attachment>>,
  /// The todo list of each turn whose agent keeps one, by the message that started the turn.
  pub todos: BTreeMap<ChatMessageId, TodoList>,
  /// The entries of its waits and runs' ends that still stand as they were read: an agent that still waits, a run
  /// still where it ended. The others were dealt with since, here or elsewhere.
  pub standing: HashSet<i64>,
  /// What became of the runs whose ends no longer stand, by entry.
  pub moved: HashMap<i64, EndMoved>,
  /// What the lines of its events of tasks' runs name beside their waits, by entry.
  pub events: HashMap<i64, EventFacts>,
  /// How full the context of the session its turns continue is; `None` while no turn's CLI reported it (unknown, not
  /// empty).
  pub context: Option<ContextUse>,
}

/// What the line of an event of a task's run names beside its wait ([`super::events`]): the module its task is in, the
/// branch the run merges into, and the task the run waits on; each `None` when there is none or the project no longer
/// keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventFacts {
  /// The module's name.
  pub module: Option<String>,
  pub branch: Option<String>,
  pub waited: Option<WaitedTask>,
}

/// A task a run waits on: its key as people read it (`KON-52`), and the number of its run the event is about, when it
/// names one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitedTask {
  pub key: String,
  pub number: Option<u32>,
}

/// What became of a run since an end of it a conversation keeps, once that end no longer stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndMoved {
  /// Orchester merged it.
  Merged,
  /// It works again, or waits to.
  Working,
  /// It stopped for a person's decision on how it goes on.
  Stopped,
  /// It is paused.
  Paused,
  /// It was interrupted.
  Interrupted,
  /// It succeeded.
  Succeeded,
  /// It failed.
  Failed,
  /// It was canceled.
  Cancelled,
  /// The project no longer keeps it.
  Gone,
}

impl EndMoved {
  /// A run that stands at `status` now.
  pub(super) const fn of(status: &RunStatus) -> Self {
    match status {
      RunStatus::Pending | RunStatus::Running => Self::Working,
      RunStatus::Paused(_) => Self::Paused,
      RunStatus::WaitingForPerson(_) => Self::Stopped,
      RunStatus::Interrupted(_) => Self::Interrupted,
      RunStatus::Succeeded => Self::Succeeded,
      RunStatus::Failed(_) => Self::Failed,
      RunStatus::Cancelled => Self::Cancelled,
    }
  }
}

impl ChatView {
  /// Conversation `chat` of the project at `root`; `None` when it is not kept. Blocks on SQLite.
  pub fn load_in(root: &Path, chat: ChatId) -> Result<Option<Self>, Message> {
    let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
    let Some(found) = db.chat(chat).map_err(|error| messages::store_error(&error))? else {
      return Ok(None);
    };
    let entries = db.chat_entries(chat).map_err(|error| messages::store_error(&error))?;
    let attachments = db
      .chat_attachments(chat)
      .map_err(|error| messages::store_error(&error))?;
    let todos = db.chat_todos(chat).map_err(|error| messages::store_error(&error))?;
    let (standing, moved) =
      super::waits::standing(&db, chat, &entries).map_err(|error| messages::store_error(&error))?;
    let events = super::events::event_facts(&db, &entries).map_err(|error| messages::store_error(&error))?;
    let context = db.chat_context(chat).map_err(|error| messages::store_error(&error))?;
    Ok(Some(Self {
      chat: found,
      entries,
      attachments,
      todos: todos.into_iter().collect(),
      standing,
      moved,
      events,
      context,
    }))
  }

  /// What became of the run of the end kept as entry `seq`, once that end no longer stands.
  pub fn moved_of(&self, seq: i64) -> Option<EndMoved> {
    self.moved.get(&seq).copied()
  }

  /// What the line of the event kept as entry `seq` names beside its wait; `None` for an entry that is no event.
  pub fn event_of(&self, seq: i64) -> Option<&EventFacts> {
    self.events.get(&seq)
  }

  /// The files attached to the person's message `message`.
  pub fn attachments_of(&self, message: ChatMessageId) -> &[Attachment] {
    self.attachments.get(&message).map_or(&[], Vec::as_slice)
  }

  /// The todo list of the turn `message` started, as its agent's latest update left it; `None` when it keeps none.
  pub fn todo_of(&self, message: ChatMessageId) -> Option<&TodoList> {
    self.todos.get(&message)
  }

  /// The last turn and where it stands: a person's message's, or one a wait of an agent started by itself.
  pub fn last_turn(&self) -> Option<(ChatMessageId, &TurnState)> {
    self.entries.iter().rev().find_map(|entry| entry.kind.turn())
  }

  /// Whether the agent is answering the last message.
  pub fn thinking(&self) -> bool {
    self.last_turn().is_some_and(|(_, turn)| turn.is_thinking())
  }

  /// Whether the session is being compacted (ORC-309).
  pub fn compacting(&self) -> bool {
    self
      .entries
      .iter()
      .any(|entry| matches!(&entry.kind, EntryKind::Event(event) if event.is_compacting()))
  }
}
