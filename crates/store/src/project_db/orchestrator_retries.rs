//! The turns of a wait that failed (`orchester_core::orchestrator::retry`): a wait whose turn failed gets its next
//! turn in the same entry, which keeps how the earlier ones ended, so the turn it starts is told and the conversation
//! can say which try it is.

use orchester_core::{
  id::{ChatId, ChatMessageId},
  orchestrator::{FailedTry, retry_note},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::{
  ProjectDb,
  orchestrator::wait_of,
  orchestrator_waits::{KeptWait, LEAD_THINKS, NO_TURN_UNDER_WAY},
};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// A wait that starts a turn by itself and whose latest turn failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedWait {
  pub kept: KeptWait,
  /// The turn its entry holds.
  pub last: FailedTry,
  /// The failed turns before it, oldest first.
  pub earlier: Vec<FailedTry>,
}

impl ProjectDb {
  /// The waits of every conversation that start a turn by themselves (`automatic`) and whose latest turn failed,
  /// oldest first.
  pub fn automatic_waits_failed(&self) -> Result<Vec<FailedWait>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT chat_id, seq, wait_key, body, failure, ended_at, tries FROM orchestrator_entries
       WHERE kind = 'wait' AND turn = 'failed' AND json_extract(body, '$.automatic') = 1 ORDER BY seq",
    )?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          [
            row.get::<_, String>(0)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
          ],
          row.get::<_, i64>(1)?,
          row.get::<_, String>(4)?,
          row.get::<_, Option<i64>>(5)?,
          row.get::<_, Option<String>>(6)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|([chat, key, body], seq, failure, ended, tries)| {
        Ok(FailedWait {
          kept: KeptWait {
            chat: parse("chat id", &chat)?,
            seq,
            key,
            wait: wait_of(&body)?,
          },
          last: failed_try(&failure, ended)?,
          earlier: tries_of(tries.as_deref())?,
        })
      })
      .collect()
  }

  /// The failed turns the waits of `chat` keep from before the turn each holds now, oldest first, by entry; a wait
  /// none of whose earlier turns failed is not listed. The conversation says which try a turn is by them.
  pub fn chat_wait_tries(&self, chat: ChatId) -> Result<Vec<(i64, Vec<FailedTry>)>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT seq, tries FROM orchestrator_entries
       WHERE chat_id = ?1 AND kind = 'wait' AND tries IS NOT NULL ORDER BY seq",
    )?;
    let rows = statement
      .query_map([chat.to_string()], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(seq, tries)| Ok((seq, tries_of(Some(&tries))?)))
      .collect()
  }

  /// Gives wait `seq` of `chat`, whose turn failed, its next turn, thinking, under `message`, and keeps the failed one
  /// among its earlier tries; under the conditions a first turn starts under ([`Self::start_wait_turn`]). The failed
  /// turn is no turn any more, so the todo list it kept goes with it. Whether it was given.
  pub fn retry_wait_turn(&mut self, entry: (ChatId, i64), message: ChatMessageId) -> Result<bool, StoreError> {
    self.retry_as(entry, message, None)
  }

  /// [`Self::retry_wait_turn`] for a wait the turn `lead`, which thinks now, takes with it
  /// ([`Self::join_wait_turn`]): its next turn is that one, under `message`.
  pub fn rejoin_wait_turn(
    &mut self,
    entry: (ChatId, i64),
    (lead, message): (ChatMessageId, ChatMessageId),
  ) -> Result<bool, StoreError> {
    self.retry_as(entry, message, Some(lead))
  }

  /// Gives wait `seq` of `chat`, whose turn failed, its next turn under `message`: its own, or the turn `lead` that
  /// takes it.
  fn retry_as(
    &mut self,
    (chat, seq): (ChatId, i64),
    message: ChatMessageId,
    lead: Option<ChatMessageId>,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let failed = transaction
      .query_row(
        "SELECT failure, ended_at, tries, message_id FROM orchestrator_entries
         WHERE seq = ?2 AND chat_id = ?1 AND kind = 'wait' AND turn = 'failed'",
        params![chat.to_string(), seq],
        |row| {
          Ok((
            [row.get::<_, String>(0)?, row.get::<_, String>(3)?],
            row.get::<_, Option<i64>>(1)?,
            row.get::<_, Option<String>>(2)?,
          ))
        },
      )
      .optional()?;
    let Some(([failure, failed_turn], ended, tries)) = failed else {
      return Ok(false);
    };
    let mut tries = tries_of(tries.as_deref())?;
    tries.push(failed_try(&failure, ended)?);
    let tries = serde_json::to_string(&tries).map_err(|error| corrupt("chat wait tries", error))?;
    // The todo list names its turn by the message the entry is about to lose, and holds that message in place.
    transaction.execute("DELETE FROM orchestrator_todos WHERE message_id = ?1", [failed_turn])?;
    let free = if lead.is_some() { LEAD_THINKS } else { NO_TURN_UNDER_WAY };
    let changed = transaction.execute(
      &format!(
        "UPDATE orchestrator_entries
         SET message_id = ?3, turn = 'thinking', failure = NULL, ended_at = NULL, tries = ?5, taken_with = ?4
         WHERE seq = ?2 AND chat_id = ?1 AND kind = 'wait' AND turn = 'failed' AND {free}"
      ),
      params![
        chat.to_string(),
        seq,
        message.to_string(),
        lead.map(|lead| lead.to_string()),
        tries
      ],
    )?;
    // A turn that was not given leaves the failed one as it was, its todo list too: the transaction is dropped.
    if changed == 1 {
      transaction.commit()?;
    }
    Ok(changed == 1)
  }

  /// `prompt`, the prompt of the turn wait `seq` started by itself, told first how its latest earlier turn ended
  /// ([`retry_note`]) when it had one.
  pub(super) fn retried(&self, seq: i64, prompt: String) -> Result<String, StoreError> {
    let tries: Option<String> = self
      .connection
      .query_row("SELECT tries FROM orchestrator_entries WHERE seq = ?1", [seq], |row| {
        row.get(0)
      })
      .optional()?
      .flatten();
    Ok(match tries_of(tries.as_deref())?.last() {
      Some(last) => format!("{}\n\n{prompt}", retry_note(last)),
      None => prompt,
    })
  }
}

/// The failed turn an entry's `failure` and `ended_at` columns hold.
fn failed_try(failure: &str, ended: Option<i64>) -> Result<FailedTry, StoreError> {
  Ok(FailedTry {
    failure: serde_json::from_str(failure).map_err(|error| corrupt("chat turn failure", error))?,
    ended: ended.map(Timestamp::from_unix_millis),
  })
}

/// The earlier failed turns an entry's `tries` column holds; none while it is NULL.
fn tries_of(tries: Option<&str>) -> Result<Vec<FailedTry>, StoreError> {
  tries.map_or_else(
    || Ok(Vec::new()),
    |tries| serde_json::from_str(tries).map_err(|error| corrupt("chat wait tries", error)),
  )
}
