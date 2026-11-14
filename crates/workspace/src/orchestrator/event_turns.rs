//! What became of the turns the events of a conversation start by themselves, as the conversation says it under each
//! event (`docs/plan/orchestrator-autonomy/OA-02-retry-clock.md`, `OA-03-…`, `frames.md` A): a turn that failed and
//! is tried again, at what time and which try that is; the try that thinks now; an event left to the person once
//! nothing more starts by itself; and a turn that waits for what holds the turns to end ([`super::held`]).
//!
//! It says what the follower will do ([`super::turns`]), read the same way: a failed turn is tried again only while
//! its wait still stands and the chat drives with something to run on; while the daily budget is used or the
//! Orchestrator's account is at its plan limit every such turn waits for that instead. An event that will start
//! nothing by itself and is not left to the person (the switch is off, or it no longer waits) has no state here: the
//! conversation shows how its last turn ended.

use std::collections::{HashMap, HashSet};

use orchester_core::{
  id::ChatId,
  orchestrator::{ChatEntry, EntryKind, FailedTry, NextTry, RETRY_AFTER, TurnFailure, TurnState, WaitTurn},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

pub use super::held::Hold;
use super::held::hold;
use crate::messages::{self, Message};

/// Where the turn of one event stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventTurn {
  /// Its turn failed as `why`, and try `next` of `of` starts from `at`.
  TriedAgain {
    at: Timestamp,
    next: usize,
    of: usize,
    why: TurnFailure,
  },
  /// Its turn thinks now, try `number` of `of`, after the ones before it failed.
  Trying { number: usize, of: usize },
  /// No further turn starts by itself: `tries` turns failed, the last as `why`. *Try again* is the person's
  /// ([`super::turns::try_again_in`]).
  LeftToPerson { tries: usize, why: TurnFailure },
  /// Its turn starts once what holds the turns ended.
  Held(Hold),
}

/// The events of one conversation whose turn has something to say, by entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventTurns {
  by_entry: HashMap<i64, EventTurn>,
}

impl EventTurns {
  /// Where the turn of the event kept as entry `seq` stands; `None` for an entry with nothing to say of it.
  pub fn of(&self, seq: i64) -> Option<&EventTurn> {
    self.by_entry.get(&seq)
  }

  /// What holds the turns of the conversation's events now; `None` while none of them waits for that.
  pub fn held(&self) -> Option<Hold> {
    self.by_entry.values().find_map(|turn| match turn {
      EventTurn::Held(hold) => Some(*hold),
      _ => None,
    })
  }
}

/// How many turns an event gets by itself: the first and one after each wait of [`RETRY_AFTER`]. A try the person
/// asked for after them counts on.
const fn tries_of(number: usize) -> usize {
  let by_itself = RETRY_AFTER.len() + 1;
  if number > by_itself { number } else { by_itself }
}

/// An event whose turn would start by itself at some time: its first, or the one after a failed turn.
enum Waiting {
  First,
  Again {
    at: Timestamp,
    earlier: usize,
    why: TurnFailure,
  },
}

/// Reads where the turns of the events among `entries`, conversation `chat`'s, stand at `now`; `standing` are its
/// waits that still stand, and `ready` says whether the chat drives and has something to run on, asked only when a
/// turn waits to start. Blocks on SQLite.
pub(super) fn read(
  db: &ProjectDb,
  (chat, entries): (ChatId, &[ChatEntry]),
  standing: &HashSet<i64>,
  (now, ready): (Timestamp, impl FnOnce() -> Result<bool, Message>),
) -> Result<EventTurns, Message> {
  let stored = |error: StoreError| messages::store_error(&error);
  let tries: HashMap<i64, Vec<FailedTry>> = db.chat_wait_tries(chat).map_err(stored)?.into_iter().collect();
  let mut by_entry = HashMap::new();
  let mut waiting = Vec::new();
  for entry in entries {
    let Some((turn, is_wait)) = automatic_turn(&entry.kind) else {
      continue;
    };
    let stands = !is_wait || standing.contains(&entry.seq);
    let earlier = tries.get(&entry.seq).map_or(0, Vec::len);
    match turn {
      None if stands => waiting.push((entry.seq, Waiting::First)),
      Some(turn) if turn.state == TurnState::Thinking && earlier > 0 => {
        let number = earlier + 1;
        let of = tries_of(number);
        by_entry.insert(entry.seq, EventTurn::Trying { number, of });
      }
      // Only a wait's turn is tried again, and only while the wait stands.
      Some(turn) if is_wait && stands => match after_failure(turn, earlier) {
        Some((NextTry::From(at), why)) => waiting.push((entry.seq, Waiting::Again { at, earlier, why })),
        Some((NextTry::LeftToPerson, why)) => {
          let tries = earlier + 1;
          by_entry.insert(entry.seq, EventTurn::LeftToPerson { tries, why });
        }
        None => {}
      },
      None | Some(_) => {}
    }
  }
  // Counting the day's tokens is not free: the hold is read only while a turn waits to start.
  if !waiting.is_empty() && ready()? {
    let held = hold(db, now).map_err(stored)?;
    for (seq, waits) in waiting {
      let turn = match (held, waits) {
        (Some(hold), _) => Some(EventTurn::Held(hold)),
        (None, Waiting::Again { at, earlier, why }) => {
          let next = earlier + 2;
          let of = tries_of(next);
          Some(EventTurn::TriedAgain { at, next, of, why })
        }
        (None, Waiting::First) => None,
      };
      by_entry.extend(turn.map(|turn| (seq, turn)));
    }
  }
  Ok(EventTurns { by_entry })
}

/// What follows `turn`, a wait's turn that failed after `earlier` failed ones, and how it failed; `None` for a turn
/// that did not fail.
fn after_failure(turn: &WaitTurn, earlier: usize) -> Option<(NextTry, TurnFailure)> {
  let TurnState::Failed(failure) = &turn.state else {
    return None;
  };
  let last = FailedTry {
    failure: failure.clone(),
    ended: turn.ended,
  };
  Some((NextTry::after(&last, earlier), last.failure))
}

/// The turn of an entry that starts one by itself (`None` inside: it has none yet), and whether the entry is a wait;
/// `None` for an entry that starts none, or was kept while the chat did not drive.
fn automatic_turn(kind: &EntryKind) -> Option<(Option<&WaitTurn>, bool)> {
  let (automatic, turn, is_wait) = match kind {
    EntryKind::Wait { wait, turn } => (wait.automatic, turn, true),
    EntryKind::Report { report, turn } => (report.automatic, turn, false),
    EntryKind::WatchEnded { summary, turn } => (summary.automatic, turn, false),
    EntryKind::Standing { event, turn } => (event.automatic, turn, false),
    EntryKind::Person { .. }
    | EntryKind::Reply { .. }
    | EntryKind::Action(_)
    | EntryKind::Event(_)
    | EntryKind::Plan(_) => return None,
  };
  automatic.then_some((turn.as_ref(), is_wait))
}
