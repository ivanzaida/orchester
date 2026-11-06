//! The person's watch commands in an Orchestrator conversation (`docs/design/orchestrator-watch.md`, frames 04 `OKtX2`
//! and 05 `EIjDH`, lines `avDZO`): `/watch-task` and `/watch-module` ([`watch_in`]), *Take it here* and *Open that
//! conversation* on the card of a target another conversation leads ([`take_over_in`], [`leave_there_in`]), and
//! `/unwatch` or × on a header chip ([`stop_in`]). The app applies them itself, with no agent turn and nothing spent,
//! also while a turn of the conversation is under way: each keeps the person's command and its lines as entries of the
//! conversation, and the line another conversation is owed in that one. The agent has no tool for any of this; only
//! the person subscribes. A target closed or deleted meanwhile is refused with [`WatchError`], whose
//! [`WatchError::message`] the composer shows.

mod read;
mod start;
mod stop;
mod take;

use orchester_core::{
  orchestrator::{
    Chat, ChatEvent,
    watch::{NamedTarget, News, Told, WatchLine},
  },
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

pub use self::{
  start::{Watched, watch_in},
  stop::{StopBy, StopWhat, Unwatched, stop_in},
  take::{TakenHere, leave_there_in, take_over_in},
};
use crate::messages::{self, Message};

/// Why a watch command was not applied; nothing changed then.
#[derive(Debug, Error)]
pub enum WatchError {
  /// The command named no target.
  #[error("the command names no target")]
  NoTarget,
  /// Every target it named was closed or deleted meanwhile (a module closed, a task deleted).
  #[error("the watch targets were closed or deleted meanwhile: {0:?}")]
  Gone(Vec<NamedTarget>),
  /// The conversation does not lead what it stops.
  #[error("the conversation does not watch that target")]
  NotWatched,
  /// Another conversation, `chat` titled `title`, leads what this one stops.
  #[error("another conversation leads that target")]
  LedElsewhere {
    chat: orchester_core::id::ChatId,
    title: String,
  },
  /// *Take it here* of a target the conversation leads already.
  #[error("the conversation leads that target already")]
  AlreadyHere,
  #[error(transparent)]
  Store(#[from] StoreError),
}

impl WatchError {
  /// What the composer shows.
  pub fn message(&self) -> Message {
    match self {
      Self::NoTarget => Message::new("orchestrator.watch.noTarget"),
      Self::Gone(targets) => {
        let names: Vec<&str> = targets.iter().filter_map(|named| named.name.as_deref()).collect();
        Message::new("orchestrator.watch.gone").with("targets", names.join(", "))
      }
      Self::NotWatched => Message::new("orchestrator.watch.notWatched"),
      Self::LedElsewhere { title, .. } => Message::new("orchestrator.watch.ledElsewhere").with("title", title.clone()),
      Self::AlreadyHere => Message::new("orchestrator.watch.alreadyHere"),
      Self::Store(error) => messages::store_error(error),
    }
  }
}

/// The conversation as kept, titled as History shows it; `chat` itself while it is not kept yet.
fn kept(db: &ProjectDb, chat: &Chat) -> Result<Chat, StoreError> {
  Ok(db.chat(chat.id())?.unwrap_or_else(|| chat.clone()))
}

/// Writes the line `told` is owed about `target` into its conversation, after a change `here` made.
fn tell(db: &mut ProjectDb, (told, target): (Told, NamedTarget), here: &Chat, at: Timestamp) -> Result<(), StoreError> {
  let line = match told.news() {
    News::TakenOver { .. } | News::Loses { .. } => WatchLine::NowLedIn {
      target,
      chat: here.id(),
      title: here.title().to_owned(),
      taken_over: matches!(told.news(), News::TakenOver { .. }),
    },
    News::Regains { .. } => WatchLine::LedHereAgain {
      target,
      from: here.id(),
      title: here.title().to_owned(),
    },
  };
  // A deleted conversation's subscriptions end with it, so a conversation that is gone has nothing to be told.
  let Some(other) = db.chat(told.chat())? else {
    return Ok(());
  };
  db.keep_watch_entries(&other, &[ChatEvent::Watch { line }], at)
}
