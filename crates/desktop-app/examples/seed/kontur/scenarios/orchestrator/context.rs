//! The `orchestrator` scenario with a reading of how full the session of *Plan the webhook retry work*, the
//! conversation the Orchestrator opens on, is (`Orchestrator · redesign` `hujxu`, `docs/design/context-meter.md`), each
//! of a window the CLI reported for `claude-opus-5-5` but the last:
//! - `orchestrator-context-plenty` 21% (42k of 200k), `-near-full` 86% (172k, `06` `Z2dtIW`), `-full` *Full* (200k) and
//!   `-unknown`, 64k of a window nobody reported (the states and popovers of `Kp4IZ`);
//! - `orchestrator-compacting`: a person's *Compact* under way from 182k (`06b` `j7eSQO`). A runner ends a compaction
//!   it does not carry out as interrupted once it looks after the project, so a QA copy shows it running only without
//!   one;
//! - `orchestrator-compacted`: the compaction done, *182k → 41k*, and the meter back to 21% (`06c` `ifYTL`): the
//!   reading after it is 42k, as 41k would read 20%;
//! - `orchestrator-compact-failed`: the compaction failed as the CLI said why; the session keeps its 182k.

use orchester_core::{
  connection::context_use::ContextUse,
  id::ChatId,
  orchestrator::{CompactionState, TurnFailure},
};

use crate::kontur::{Kontur, SeedError};

/// The window of the Orchestrator's model, as its CLI reports it.
const WINDOW: u64 = 200_000;
/// The tokens the session held when the person compacted it, and after.
const BEFORE: u64 = 182_000;
const AFTER: u64 = 41_000;
/// The reading after the compaction: 21% of the window.
const AFTER_READING: u64 = 42_000;

pub fn plenty(kontur: &mut Kontur) -> Result<(), SeedError> {
  with_reading(kontur, ContextUse::reported(AFTER_READING, WINDOW)).map(drop)
}

pub fn near_full(kontur: &mut Kontur) -> Result<(), SeedError> {
  with_reading(kontur, ContextUse::reported(172_000, WINDOW)).map(drop)
}

pub fn full(kontur: &mut Kontur) -> Result<(), SeedError> {
  with_reading(kontur, ContextUse::reported(WINDOW, WINDOW)).map(drop)
}

pub fn unknown(kontur: &mut Kontur) -> Result<(), SeedError> {
  with_reading(kontur, ContextUse::unknown_window(64_000)).map(drop)
}

pub fn compacting(kontur: &mut Kontur) -> Result<(), SeedError> {
  compaction(kontur).map(drop)
}

pub fn compacted(kontur: &mut Kontur) -> Result<(), SeedError> {
  let seq = compaction(kontur)?;
  let done = CompactionState::Done { after: Some(AFTER) };
  end(kontur, seq, done, Some(&ContextUse::reported(AFTER_READING, WINDOW)))
}

pub fn compact_failed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let seq = compaction(kontur)?;
  let failed = CompactionState::Failed {
    reason: TurnFailure::Failed {
      reason: "claude exited with code 1: the session could not be summarised".to_owned(),
    },
  };
  end(kontur, seq, failed, None)
}

/// The `orchestrator` scenario, the conversation it opens on keeping `reading` as its session's latest; that
/// conversation.
fn with_reading(kontur: &mut Kontur, reading: ContextUse) -> Result<ChatId, SeedError> {
  super::seed(kontur)?;
  let chat = kontur
    .db
    .latest_chat()?
    .ok_or_else(|| SeedError::Invalid("the Orchestrator's conversation".to_owned()))?;
  kontur.db.set_chat_context(chat, &reading)?;
  Ok(chat)
}

/// The conversation at [`BEFORE`] tokens, compacted by the person five minutes ago, still running; its entry.
fn compaction(kontur: &mut Kontur) -> Result<i64, SeedError> {
  let chat = with_reading(kontur, ContextUse::reported(BEFORE, WINDOW))?;
  let at = kontur.ago(5);
  Ok(kontur.db.begin_chat_compaction(chat, Some(BEFORE), at)?)
}

/// Ends compaction `seq` as `state`, leaving the session `reading` when it is done.
fn end(kontur: &mut Kontur, seq: i64, state: CompactionState, reading: Option<&ContextUse>) -> Result<(), SeedError> {
  if kontur.db.end_chat_compaction(seq, state, reading)? {
    Ok(())
  } else {
    Err(SeedError::Invalid(format!("compaction {seq} had ended")))
  }
}
