//! `/unwatch` and × on a header chip: the person ends the conversation's subscriptions at once. It cancels and pauses
//! nothing: the runs of their tasks go on as they are, and the stop line lists what was left open. A conversation
//! whose less specific subscription covers a stopped target again is told so in a line.

use std::path::Path;

use orchester_core::{
  orchestrator::{
    Chat, ChatEvent, Subscriptions, WatchTarget,
    watch::{NamedTarget, OpenTask, WatchCommand, WatchLine},
  },
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{WatchError, kept, read, tell};

/// What a stop ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopWhat {
  /// One target, as the chip names it.
  Target(NamedTarget),
  /// Everything the conversation watches.
  All,
}

/// How the person stopped: with `/unwatch`, whose command is kept as their entry, or with × on a header chip, which
/// leaves only the stop line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopBy {
  Unwatch,
  Chip,
}

/// What a stop ended, and what it left open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unwatched {
  pub stopped: Vec<NamedTarget>,
  pub left_open: Vec<OpenTask>,
}

/// `chat` stops watching `what` in the project at `root`. Refused when it watches none of it, or another
/// conversation leads the target by now. Blocks on SQLite.
pub fn stop_in(root: &Path, chat: &Chat, what: &StopWhat, by: StopBy) -> Result<Unwatched, WatchError> {
  let mut db = ProjectDb::open(root)?;
  let now = Timestamp::now();
  let mut set = db.active_subscriptions()?;
  let targets = chosen(&db, chat, &set, what)?;
  let plain: Vec<WatchTarget> = targets.iter().map(|named| named.target).collect();
  let left_open = read::left_open(&db, chat.id(), &set, &plain)?;
  let mut stopped = Vec::new();
  let mut told = Vec::new();
  for named in targets {
    let module = read::find(&db, named.target)?.and_then(|found| found.module);
    let Some(change) = set.stop(chat.id(), named.target, module) else {
      continue;
    };
    // Another change ended it meanwhile: there is nothing left to stop.
    if db.stop_subscription(chat.id(), named.target, now)?.is_none() {
      continue;
    }
    if let Some(owed) = change.told() {
      told.push((owed, named.clone()));
    }
    set = change.into_set();
    stopped.push(named);
  }
  if stopped.is_empty() {
    return Err(WatchError::NotWatched);
  }
  let mut entries = Vec::new();
  if by == StopBy::Unwatch {
    let command = match what {
      StopWhat::Target(_) => WatchCommand::Unwatch {
        targets: stopped.clone(),
      },
      StopWhat::All => WatchCommand::UnwatchAll,
    };
    entries.push(ChatEvent::WatchCommand { command });
  }
  let line = WatchLine::Stopped {
    targets: stopped.clone(),
    left_open: left_open.clone(),
  };
  entries.push(ChatEvent::Watch { line });
  db.keep_watch_entries(chat, &entries, now)?;
  let here = kept(&db, chat)?;
  for owed in told {
    tell(&mut db, owed, &here, now)?;
  }
  Ok(Unwatched { stopped, left_open })
}

/// The targets `what` names that `chat` leads in `set`, named as they are now.
fn chosen(db: &ProjectDb, chat: &Chat, set: &Subscriptions, what: &StopWhat) -> Result<Vec<NamedTarget>, WatchError> {
  let targets = match what {
    StopWhat::All => set
      .of_chat(chat.id())
      .map(|led| {
        read::renamed(
          db,
          &NamedTarget {
            target: led.target(),
            name: None,
          },
        )
      })
      .collect::<Result<Vec<_>, StoreError>>()?,
    StopWhat::Target(given) => match set.of(given.target) {
      Some(led) if led.chat() == chat.id() => vec![read::renamed(db, given)?],
      Some(led) => {
        return Err(WatchError::LedElsewhere {
          chat: led.chat(),
          title: read::title(db, led.chat())?,
        });
      }
      None => Vec::new(),
    },
  };
  if targets.is_empty() {
    return Err(WatchError::NotWatched);
  }
  Ok(targets)
}
