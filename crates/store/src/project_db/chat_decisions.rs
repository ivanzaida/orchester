//! Decisions on the Orchestrator chat's proposals (`orchester_core::orchestrator::ChatAction`): a dismissal settles a
//! proposal that waits; a person's start or apply first claims it, then settles it once carried out; and the claims
//! no settlement followed are found again. Each change is one transaction, so two decisions never both take a
//! proposal.

use orchester_core::{
  id::{ChatId, RunId},
  orchestrator::{ActionOutcome, ChatAction},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

impl ProjectDb {
  /// Settles the proposed action `seq` of conversation `chat` with `outcome` at `at`, e.g. dismissed: the action as
  /// it was, or `None` when there is no such action or it no longer waits for a person.
  pub fn settle_chat_action(
    &mut self,
    (chat, seq): (ChatId, i64),
    outcome: ActionOutcome,
    at: Timestamp,
  ) -> Result<Option<ChatAction>, StoreError> {
    self.replace_chat_action((chat, seq), ChatAction::waits, |found| ChatAction {
      outcome,
      settled: Some(at),
      ..found
    })
  }

  /// Claims the proposed action `seq` of conversation `chat` for a person's decision at `at`: it is
  /// [`ChatAction::claimed`] until [`Self::finish_chat_action`]. The action as it was, or `None` when it no longer
  /// waits for a person, e.g. because another decision claimed it first.
  pub fn claim_chat_action(
    &mut self,
    (chat, seq): (ChatId, i64),
    at: Timestamp,
  ) -> Result<Option<ChatAction>, StoreError> {
    self.replace_chat_action((chat, seq), ChatAction::waits, |found| ChatAction {
      claimed: Some(at),
      ..found
    })
  }

  /// Every action a decision claimed before `before` and did not settle: in conversation order, each with its
  /// conversation and entry.
  pub fn claimed_chat_actions(&self, before: Timestamp) -> Result<Vec<(ChatId, i64, ChatAction)>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT chat_id, seq, body FROM orchestrator_entries WHERE kind = 'action' ORDER BY seq")?;
    let rows = statement.query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, i64>(1)?,
        row.get::<_, String>(2)?,
      ))
    })?;
    let mut claimed = Vec::new();
    for row in rows {
      let (chat, seq, body) = row?;
      let action: ChatAction = serde_json::from_str(&body).map_err(|error| corrupt("chat action", error))?;
      if action.claimed.is_some_and(|at| at < before) {
        claimed.push((parse("chat id", &chat)?, seq, action));
      }
    }
    Ok(claimed)
  }

  /// Ends the claimed action `seq` of conversation `chat` with `outcome` at `at`; `None` when it was not claimed.
  pub fn finish_chat_action(
    &mut self,
    (chat, seq): (ChatId, i64),
    outcome: ActionOutcome,
    at: Timestamp,
  ) -> Result<Option<ChatAction>, StoreError> {
    self.replace_chat_action(
      (chat, seq),
      |found| found.claimed.is_some(),
      |found| ChatAction {
        outcome,
        settled: Some(at),
        claimed: None,
        ..found
      },
    )
  }

  /// Keeps on the settled action `seq` of `chat` the run that carrying it out started (a message that ran its task
  /// again); the action as it was, or `None` when it is not settled.
  pub fn keep_chat_action_start(
    &mut self,
    (chat, seq): (ChatId, i64),
    run: RunId,
  ) -> Result<Option<ChatAction>, StoreError> {
    self.replace_chat_action(
      (chat, seq),
      |found| found.settled.is_some(),
      |found| ChatAction {
        started: Some(run),
        ..found
      },
    )
  }

  /// Replaces action `seq` of `chat` with what `change` makes of it, while `current` holds for it; in one
  /// transaction, so two decisions never both take it. The action as it was.
  fn replace_chat_action(
    &mut self,
    (chat, seq): (ChatId, i64),
    current: impl Fn(&ChatAction) -> bool,
    change: impl FnOnce(ChatAction) -> ChatAction,
  ) -> Result<Option<ChatAction>, StoreError> {
    let transaction = self
      .connection
      .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let body: Option<String> = transaction
      .query_row(
        "SELECT body FROM orchestrator_entries WHERE seq = ?1 AND chat_id = ?2 AND kind = 'action'",
        params![seq, chat.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    let Some(body) = body else {
      return Ok(None);
    };
    let found: ChatAction = serde_json::from_str(&body).map_err(|error| corrupt("chat action", error))?;
    if !current(&found) {
      return Ok(None);
    }
    let replaced = change(found.clone());
    let body = serde_json::to_string(&replaced).map_err(|error| corrupt("chat action", error))?;
    transaction.execute(
      "UPDATE orchestrator_entries SET body = ?1 WHERE seq = ?2",
      params![body, seq],
    )?;
    transaction.commit()?;
    Ok(Some(found))
  }
}
