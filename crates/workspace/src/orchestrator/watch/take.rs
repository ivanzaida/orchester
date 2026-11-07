//! The card of a target another conversation leads: *Take it here* moves the subscription here, the conversation that
//! lost it gets the line *X is now led in «…»*, and this one the line that it took it ([`take_over_in`]); *Open that
//! conversation* leaves it there and keeps the line the card settles into ([`leave_there_in`]).

use std::path::Path;

use orchester_core::{
  id::ChatId,
  orchestrator::{
    Chat, ChatEvent, Subscription,
    watch::{LedElsewhere, NamedTarget, WatchLine},
  },
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{WatchError, kept, read, tell};

/// What *Take it here* did: the conversation it took the target from; `None` when nobody led it by then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TakenHere {
  pub from: Option<ChatId>,
}

/// `chat` leads `target` from now on, following `order`, the order of the command whose card offered it, in the
/// project at `root`. Refused when the target was closed or deleted meanwhile, or `chat` leads it already. Blocks
/// on SQLite.
pub fn take_over_in(root: &Path, chat: &Chat, target: &NamedTarget, order: &str) -> Result<TakenHere, WatchError> {
  let mut db = ProjectDb::open(root)?;
  let found = read::find(&db, target.target)?.ok_or_else(|| WatchError::Gone(vec![target.clone()]))?;
  let set = db.active_subscriptions()?;
  if set.of(target.target).is_some_and(|led| led.chat() == chat.id()) {
    return Err(WatchError::AlreadyHere);
  }
  let now = Timestamp::now();
  let subscription = Subscription::new(chat.id(), target.target, order, now);
  let taken = set.take_over(subscription.clone(), found.module);
  let from = db
    .take_over_subscription(&subscription)?
    .map(|led| led.chat())
    .filter(|&led| led != chat.id());
  let named = found.named;
  let line = match from {
    Some(other) => WatchLine::TookOver {
      target: named.clone(),
      from: other,
      title: read::title(&db, other)?,
    },
    None => WatchLine::Watching {
      started: vec![named.clone()],
      already_here: Vec::new(),
      led_elsewhere: Vec::new(),
      gone: Vec::new(),
    },
  };
  db.keep_watch_entries(chat, &[ChatEvent::Watch { line }], now)?;
  if let Some(told) = taken.told() {
    let here = kept(&db, chat)?;
    tell(&mut db, (told, named), &here, now)?;
  }
  Ok(TakenHere { from })
}

/// *Open that conversation* on `chat`'s card of `led`, in the project at `root`: the target stays where it is led, and
/// `chat` keeps the line its card settles into. Blocks on SQLite.
pub fn leave_there_in(root: &Path, chat: &Chat, led: &LedElsewhere) -> Result<(), WatchError> {
  let mut db = ProjectDb::open(root)?;
  let line = WatchLine::LeftThere {
    target: led.target.clone(),
    chat: led.chat,
    title: led.title.clone(),
  };
  db.keep_watch_entries(chat, &[ChatEvent::Watch { line }], Timestamp::now())?;
  Ok(())
}
