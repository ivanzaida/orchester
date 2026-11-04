//! The Orchestrator's plan-only mode (ORC-646, `orchester_core::orchestrator::plan`): a conversation's *Plan only*
//! switch, and the plans its turns that plan answered with. A plan is an `event` entry whose body is
//! `{"kind": "plan", "plan": …}` (`schema::plan_only` says why); a newer plan of its conversation replaces the one that
//! waits, and approving one keeps, in the same transaction, the message whose turn carries it out.

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{ChatPlan, PlanState, TurnMode, TurnState, plan::carry_out_prompt},
  run::Answerer,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// A plan's body as its entry keeps it.
#[derive(Serialize, Deserialize)]
struct Kept {
  kind: String,
  plan: ChatPlan,
}

const PLAN: &str = "plan";

/// The plan an `event` entry's `body` holds; `None` for an event of another kind.
pub(super) fn plan_of(body: &str) -> Result<Option<ChatPlan>, StoreError> {
  let value: serde_json::Value = serde_json::from_str(body).map_err(|error| corrupt("chat event", error))?;
  if value.get("kind").and_then(serde_json::Value::as_str) != Some(PLAN) {
    return Ok(None);
  }
  let kept: Kept = serde_json::from_value(value).map_err(|error| corrupt("chat plan", error))?;
  Ok(Some(kept.plan))
}

fn body_of(plan: &ChatPlan) -> Result<String, StoreError> {
  serde_json::to_string(&Kept {
    kind: PLAN.to_owned(),
    plan: plan.clone(),
  })
  .map_err(|error| corrupt("chat plan", error))
}

/// A plan the person approved, and the message whose turn carries it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApprovedPlan {
  pub chat: ChatId,
  pub message: ChatMessageId,
}

impl ProjectDb {
  /// Turns conversation `chat`'s *Plan only* switch on or off; whether the conversation is kept.
  pub fn set_chat_plan_only(&mut self, chat: ChatId, on: bool) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE orchestrator_chats SET plan_only = ?2 WHERE id = ?1",
      params![chat.to_string(), on],
    )?;
    Ok(changed == 1)
  }

  /// Keeps `plan` as conversation `chat`'s newest plan at `at`; the plan of it that waited is replaced by it then.
  /// Returns its entry.
  pub fn add_chat_plan(&mut self, chat: ChatId, plan: &ChatPlan, at: Timestamp) -> Result<i64, StoreError> {
    let transaction = self.connection.transaction()?;
    for (seq, waiting) in plans_of(&transaction, chat)? {
      if waiting.waits() {
        settle(&transaction, seq, &waiting.settled(PlanState::Replaced { at }))?;
      }
    }
    transaction.execute(
      "INSERT INTO orchestrator_entries (chat_id, kind, at, body) VALUES (?1, 'event', ?2, ?3)",
      params![chat.to_string(), at.unix_millis(), body_of(plan)?],
    )?;
    let seq = transaction.last_insert_rowid();
    transaction.commit()?;
    Ok(seq)
  }

  /// The plan kept as entry `seq`, and its conversation; `None` when that entry is no plan.
  pub fn chat_plan(&self, seq: i64) -> Result<Option<(ChatId, ChatPlan)>, StoreError> {
    plan_at(&self.connection, seq)
  }

  /// Dismisses the plan kept as entry `seq` at `at`, while it waits; whether it waited.
  pub fn dismiss_chat_plan(&mut self, seq: i64, at: Timestamp) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let Some((_, plan)) = plan_at(&transaction, seq)?.filter(|(_, plan)| plan.waits()) else {
      return Ok(false);
    };
    settle(&transaction, seq, &plan.settled(PlanState::Dismissed { at }))?;
    transaction.commit()?;
    Ok(true)
  }

  /// Approves the plan kept as entry `seq` at `at`, while it waits, and keeps `message`, sent `by` the person, as
  /// the message whose turn carries it out ([`TurnMode::CarriesOut`]: it holds the plan as the agent is told it), its
  /// turn thinking. `None` when the plan no longer waits.
  pub fn approve_chat_plan(
    &mut self,
    seq: i64,
    (message, by): (ChatMessageId, Answerer),
    at: Timestamp,
  ) -> Result<Option<ApprovedPlan>, StoreError> {
    let transaction = self.connection.transaction()?;
    let Some((chat, plan)) = plan_at(&transaction, seq)?.filter(|(_, plan)| plan.waits()) else {
      return Ok(None);
    };
    let prompt = carry_out_prompt(&plan);
    let body = MessageBody::new(&prompt).map_err(|error| corrupt("chat plan", error))?;
    settle(&transaction, seq, &plan.settled(PlanState::Approved { at }))?;
    transaction.execute(
      "INSERT INTO orchestrator_entries (chat_id, kind, at, body, message_id, turn, by_automation, mode)
       VALUES (?1, 'person', ?2, ?3, ?4, ?5, ?6, ?7)",
      params![
        chat.to_string(),
        at.unix_millis(),
        body.as_str(),
        message.to_string(),
        TurnState::Thinking.word(),
        by == Answerer::Automation,
        TurnMode::CarriesOut.word()
      ],
    )?;
    transaction.commit()?;
    Ok(Some(ApprovedPlan { chat, message }))
  }

  /// Takes back an approval of the plan kept as entry `seq` whose turn no runner took: the message that would have
  /// carried it out, `message`, is forgotten and the plan waits again. Whether it was approved.
  pub fn reopen_chat_plan(&mut self, seq: i64, message: ChatMessageId) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let Some((_, plan)) =
      plan_at(&transaction, seq)?.filter(|(_, plan)| matches!(plan.state(), PlanState::Approved { .. }))
    else {
      return Ok(false);
    };
    transaction.execute(
      "DELETE FROM orchestrator_entries WHERE message_id = ?1 AND kind = 'person' AND mode = 'carry_out'
         AND turn = 'thinking'",
      [message.to_string()],
    )?;
    settle(&transaction, seq, &plan.settled(PlanState::Waits))?;
    transaction.commit()?;
    Ok(true)
  }
}

/// The plan kept as entry `seq`, and its conversation.
fn plan_at(connection: &rusqlite::Connection, seq: i64) -> Result<Option<(ChatId, ChatPlan)>, StoreError> {
  let row: Option<(String, String)> = connection
    .query_row(
      "SELECT chat_id, body FROM orchestrator_entries WHERE seq = ?1 AND kind = 'event'",
      [seq],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?;
  let Some((chat, body)) = row else {
    return Ok(None);
  };
  let Some(plan) = plan_of(&body)? else {
    return Ok(None);
  };
  Ok(Some((parse("chat id", &chat)?, plan)))
}

/// The plans of conversation `chat`, by entry.
fn plans_of(transaction: &Transaction<'_>, chat: ChatId) -> Result<Vec<(i64, ChatPlan)>, StoreError> {
  let mut statement = transaction.prepare(
    "SELECT seq, body FROM orchestrator_entries WHERE chat_id = ?1 AND kind = 'event'
       AND json_extract(body, '$.kind') = 'plan'",
  )?;
  let rows: Vec<(i64, String)> = statement
    .query_map([chat.to_string()], |row| Ok((row.get(0)?, row.get(1)?)))?
    .collect::<Result<_, _>>()?;
  rows
    .into_iter()
    .filter_map(|(seq, body)| plan_of(&body).transpose().map(|plan| plan.map(|plan| (seq, plan))))
    .collect()
}

/// Keeps entry `seq` as `plan`.
fn settle(transaction: &Transaction<'_>, seq: i64, plan: &ChatPlan) -> Result<(), StoreError> {
  transaction.execute(
    "UPDATE orchestrator_entries SET body = ?2 WHERE seq = ?1",
    params![seq, body_of(plan)?],
  )?;
  Ok(())
}
