//! Which entries of the project's conversations get the turn they start by themselves now ([`super::waits`]), oldest
//! first: the waits that have none yet, the daily reports and the ends of subscriptions, and the waits whose turn
//! failed and is tried again (`orchester_core::orchestrator::retry`,
//! `docs/plan/orchestrator-autonomy/OA-01-failed-turn-keeps-event.md`). A wait whose turn failed stays open: its next
//! turn starts in the same entry once its time came, three times at most, and only while the wait still stands; after
//! that, or after a failure only the person can lift, no turn starts by itself and the wait is the person's, where it
//! shows outside the chat. A turn the person stopped did not fail and is not tried again. While the project's daily
//! budget is used or the Orchestrator's account is at its plan limit ([`super::held`]) nothing starts by itself: the
//! entries wait for the reset. The turn a wait starts takes the next waits of its conversation that are due with it,
//! [`TOGETHER_MOST`] events at most (`docs/plan/orchestrator-autonomy/OA-07-events-together.md`): each thinks and ends
//! as that turn does, so a turn that failed leaves each of them to be tried again. A daily report, the summary of a
//! watching that ended and an event of a whole target keep turns of their own.

use std::{collections::HashSet, path::Path};

use orchester_core::{
  id::{ChatId, ChatMessageId},
  orchestrator::{AgentWait, Merging, NextTry, instructions::TOGETHER_MOST},
  time::Timestamp,
};
use orchester_store::{FailedWait, ProjectDb, StoreError};

use super::{held::held, settings::OrchestratorSetup, waits::WaitTurn};
use crate::messages::{self, Message};

/// An entry whose turn starts by itself: its conversation and `seq`, whether its turn merges a run on its own, and
/// which turn of it this is.
struct NextTurn {
  chat: ChatId,
  seq: i64,
  merges: bool,
  turn: Turn,
  /// A wait: the turn of an older wait of its conversation may take it, and its own turn takes the next ones.
  joins: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Turn {
  /// It has none yet.
  First,
  /// Its turn failed and is due again.
  Again,
  /// Its turn failed and is tried again later: nothing starts now. Only a merging turn is listed so, for the merges
  /// after it.
  Later,
}

/// The keys of the waits kept, and of those kept whose turn is still to come, a first one or one tried again: a kept
/// end that starts none needs no look again.
pub(super) fn settled(db: &ProjectDb) -> Result<(HashSet<String>, HashSet<String>), StoreError> {
  let mut pending: HashSet<String> = db
    .automatic_waits_without_turn()?
    .into_iter()
    .map(|kept| kept.key)
    .collect();
  pending.extend(
    db.automatic_waits_failed()?
      .into_iter()
      .filter(|failed| next_try(failed) != NextTry::LeftToPerson)
      .map(|failed| failed.kept.key),
  );
  Ok((db.kept_wait_keys()?, pending))
}

/// Gives the oldest entry that starts a turn by itself at `now` its turn, in its conversation: the waits that still
/// wait (their keys among `waiting`) and have no turn, or whose failed turn is due again; the daily reports
/// (`super::report`), whose turn adds a summary to what they tell; and the ends of subscriptions
/// (`super::watch_ends`), whose turn writes their module's or task's summary. `None` while a turn thinks in each of
/// their conversations, or none is due. `attached`: whether the runner is there to answer; without it nothing starts,
/// and the answer is whether a turn would.
pub(super) fn start_next(
  db: &mut ProjectDb,
  waiting: &HashSet<&str>,
  (attached, now): (bool, Timestamp),
) -> Result<Started, StoreError> {
  let next = next_turns(db, waiting, now)?;
  if next.iter().all(|next| next.turn == Turn::Later) || held(db, now)?.is_some() {
    return Ok(Started::Nothing);
  }
  if !attached {
    return Ok(Started::Pending);
  }
  Ok(start(db, &next)?.map_or(Started::Nothing, Started::Turn))
}

/// What [`start_next`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Started {
  Nothing,
  /// A turn would start, once the project's runner attached.
  Pending,
  Turn(WaitTurn),
}

/// The soonest time after `now` a turn of the project at `root` may start by the clock alone: the end of what holds
/// the turns that wait ([`super::held`]), else the soonest time a failed turn is due again; `None` while neither is.
/// The app's follower wakes then ([`super::waits::follow_in`]). Blocks on SQLite.
pub fn next_due_in(root: &Path, now: Timestamp) -> Result<Option<Timestamp>, Message> {
  let read = || {
    let db = ProjectDb::open_read_only(root)?;
    let failed = db.automatic_waits_failed()?;
    let tries: Vec<NextTry> = failed.iter().map(next_try).collect();
    let waiting = tries.iter().any(|next| *next != NextTry::LeftToPerson)
      || !db.automatic_waits_without_turn()?.is_empty()
      || !db.automatic_reports_without_turn()?.is_empty();
    // Read only while a turn waits: counting the day's tokens is not free.
    if waiting && let Some(held) = held(&db, now)? {
      return Ok(Some(held.until()));
    }
    Ok::<_, StoreError>(
      tries
        .into_iter()
        .filter_map(|next| match next {
          NextTry::From(from) if from > now => Some(from),
          NextTry::From(_) | NextTry::LeftToPerson => None,
        })
        .min(),
    )
  };
  read().map_err(|error| messages::store_error(&error))
}

/// Gives the wait kept as entry `seq` of conversation `chat` in the project at `root`, whose turn failed, one more
/// turn now, as the person asked with *Try again* (`docs/plan/orchestrator-autonomy/OA-02-retry-clock.md`): also
/// after its last try by itself, and without waiting for the time its next one is due. The turn is told how the one
/// before it ended, as any later try is. Refused, with nothing changed, while the Orchestrator has nothing to run on,
/// once the wait no longer stands, and while another turn of the conversation thinks or this one's is no longer the
/// failed one. The caller asks the runner for the turn. Blocks on SQLite.
pub fn try_again_in(root: &Path, (chat, seq): (ChatId, i64)) -> Result<WaitTurn, Message> {
  if !OrchestratorSetup::load_in(root)?.readiness().is_ready() {
    return Err(Message::new("orchestrator.send.notReady"));
  }
  let stored = |error: StoreError| messages::store_error(&error);
  let mut db = ProjectDb::open(root).map_err(stored)?;
  let entries = db.chat_entries(chat).map_err(stored)?;
  let (standing, _) = super::waits::standing(&db, chat, &entries).map_err(stored)?;
  if !standing.contains(&seq) {
    return Err(Message::new("orchestrator.tryAgain.settled"));
  }
  let message = ChatMessageId::generate();
  if !db.retry_wait_turn((chat, seq), message).map_err(stored)? {
    return Err(Message::new("orchestrator.tryAgain.busy"));
  }
  Ok(WaitTurn { chat, message })
}

fn next_try(failed: &FailedWait) -> NextTry {
  NextTry::after(&failed.last, failed.earlier.len())
}

fn next_turns(db: &ProjectDb, waiting: &HashSet<&str>, now: Timestamp) -> Result<Vec<NextTurn>, StoreError> {
  let first = db
    .automatic_waits_without_turn()?
    .into_iter()
    .filter(|kept| waiting.contains(kept.key.as_str()))
    .map(|kept| NextTurn {
      chat: kept.chat,
      seq: kept.seq,
      merges: merges(&kept.wait),
      turn: Turn::First,
      joins: true,
    });
  let again = db
    .automatic_waits_failed()?
    .into_iter()
    .filter(|failed| waiting.contains(failed.kept.key.as_str()))
    .filter_map(|failed| {
      let turn = match next_try(&failed) {
        NextTry::LeftToPerson => return None,
        next if next.is_due(now) => Turn::Again,
        NextTry::From(_) if merges(&failed.kept.wait) => Turn::Later,
        NextTry::From(_) => return None,
      };
      Some(NextTurn {
        chat: failed.kept.chat,
        seq: failed.kept.seq,
        merges: merges(&failed.kept.wait),
        turn,
        joins: true,
      })
    });
  let reports = db.automatic_reports_without_turn()?.into_iter().map(|kept| NextTurn {
    chat: kept.chat,
    seq: kept.seq,
    merges: false,
    turn: Turn::First,
    joins: false,
  });
  let mut next: Vec<NextTurn> = first.chain(again).chain(reports).collect();
  next.sort_by_key(|turn| turn.seq);
  Ok(next)
}

/// Gives the oldest of `next` whose conversation has no turn thinking its turn, in that conversation; none while a
/// turn thinks in each of theirs. Runs merge one at a time in the order they ended: a turn that merges its run on its
/// own does not start while one that ended before, in any conversation, still waits for its turn, a first one or one
/// tried again, or thinks.
fn start(db: &mut ProjectDb, next: &[NextTurn]) -> Result<Option<WaitTurn>, StoreError> {
  let mut merging_before = db.waits_thinking()?.iter().any(|kept| merges(&kept.wait));
  for (index, lead) in next.iter().enumerate() {
    if lead.merges && merging_before {
      continue;
    }
    let message = ChatMessageId::generate();
    // Refused while another turn of its conversation thinks: the entry keeps no new turn and starts once that one
    // ended.
    let started = match lead.turn {
      Turn::First => db.start_wait_turn((lead.chat, lead.seq), message)?,
      Turn::Again => db.retry_wait_turn((lead.chat, lead.seq), message)?,
      Turn::Later => false,
    };
    if started {
      if lead.joins {
        take_with(db, (lead, message), &next[index + 1..], merging_before)?;
      }
      return Ok(Some(WaitTurn {
        chat: lead.chat,
        message,
      }));
    }
    merging_before |= lead.merges;
  }
  Ok(None)
}

/// Gives the turn `message`, which the wait `lead` started, the waits of its conversation that are due after it among
/// `rest`, oldest first, until it holds [`TOGETHER_MOST`] events: each thinks and ends with that turn. A success whose
/// turn merges its run is taken only while no merge of another turn waits before it (`merging_before`), as it would
/// start; the successes one turn takes merge one after the other in it.
fn take_with(
  db: &mut ProjectDb,
  (lead, message): (&NextTurn, ChatMessageId),
  rest: &[NextTurn],
  mut merging_before: bool,
) -> Result<(), StoreError> {
  let mut taken = 1;
  for next in rest {
    if taken == TOGETHER_MOST {
      break;
    }
    let entry = (next.chat, next.seq);
    let own = ChatMessageId::generate();
    let joined = next.joins
      && next.chat == lead.chat
      && !(next.merges && merging_before)
      && match next.turn {
        Turn::First => db.join_wait_turn(entry, (message, own))?,
        Turn::Again => db.rejoin_wait_turn(entry, (message, own))?,
        Turn::Later => false,
      };
    if joined {
      taken += 1;
    } else {
      merging_before |= next.merges;
    }
  }
  Ok(())
}

/// Whether the turn of `wait` merges its run on its own.
fn merges(wait: &AgentWait) -> bool {
  wait.merging == Some(Merging::Merges)
}
