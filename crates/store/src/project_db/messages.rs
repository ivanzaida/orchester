//! Task conversations (`messages`): a person's messages, written by the desktop, and their delivery
//! and agents' messages, written by the runner. The schema keeps what a message says and lets its
//! delivery only move forward; a message sent now that was not delivered is queued again only when
//! the person sends it after the turn.

use std::num::NonZeroU64;

use orchester_core::{
  attachment::NewAttachment,
  conversation::{AgentMessage, Arrival, Delivery, Message, MessageBody, Recipient, Sender, Timing, Undelivered},
  id::{IdempotencyKey, RunId, TaskId},
  run::EventSequence,
  store::ConversationStore,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row};

use super::{
  ProjectDb,
  attachments::{self, AttachmentOwner},
  message_records::{MessageRecord, insert},
  orchestrator_sent::OrchestratorColumns,
};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

pub(super) const PERSON: &str = "person";
pub(super) const AGENT: &str = "agent";
const COLUMNS: &str = "id, task_id, run_id, author, kind, recipient_profile, step_execution_id, invocation_id, body, \
                       created_at, delivered_at, acknowledged_at, recovery_sequence, timing, arrival, \
                       undelivered_execution, undelivered_at, undelivered_reason, by_orchestrator, by_automation, \
                       orchestrator_chat";

impl ProjectDb {
  /// Stores a person's `message` with `attachments` once per `key`, as [`ConversationStore::send_message`] does: a
  /// retry returns the message stored the first time, with the attachments it was stored with. The copies of the
  /// attachments are written before the rows, and removed again when the message is not stored.
  pub fn send_message_with(
    &mut self,
    key: IdempotencyKey,
    message: &Message,
    attachments: &[NewAttachment],
  ) -> Result<Message, StoreError> {
    let transaction = self.connection.transaction()?;
    let existing = transaction
      .query_row(
        &format!("SELECT {COLUMNS} FROM messages WHERE idempotency_key = ?1"),
        [key.to_string()],
        StoredRow::read,
      )
      .optional()?;
    if let Some(existing) = existing {
      return existing.into_message();
    }
    super::task_identities::ensure_known(&transaction, message.task())?;
    let (written, kept) = attachments::write(&self.directory, attachments)?;
    insert(&transaction, message, Some(key))?;
    attachments::insert(
      &transaction,
      AttachmentOwner::Message(message.id()),
      &kept,
      message.created_at(),
    )?;
    transaction.commit()?;
    written.keep();
    Ok(message.clone())
  }
}

impl ConversationStore for ProjectDb {
  type Error = StoreError;

  fn send_message(&mut self, key: IdempotencyKey, message: &Message) -> Result<Message, StoreError> {
    self.send_message_with(key, message, &[])
  }

  fn add_agent_message(&mut self, message: &Message) -> Result<(), StoreError> {
    self.write_message(&MessageRecord::added(message)).map(drop)
  }

  fn save_deliveries(&mut self, messages: &[Message]) -> Result<(), StoreError> {
    self.write_message(&MessageRecord::deliveries(messages)?).map(drop)
  }

  fn change_delivery(&mut self, before: &Message, changed: &Message) -> Result<bool, StoreError> {
    self.write_message(&MessageRecord::delivery_change(before, changed)?)
  }

  fn messages(&self, task: TaskId) -> Result<Vec<Message>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM messages WHERE task_id = ?1 ORDER BY created_at, rowid"
    ))?;
    let rows = statement
      .query_map([task.to_string()], StoredRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(StoredRow::into_message).collect()
  }

  fn runs_with_queued_messages(&self) -> Result<Vec<RunId>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT run_id FROM messages WHERE author = ?1 AND delivered_at IS NULL AND undelivered_at IS NULL \
       AND run_id IS NOT NULL GROUP BY run_id ORDER BY MIN(created_at), MIN(rowid)",
    )?;
    let runs = statement
      .query_map([PERSON], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    runs.iter().map(|run| parse("run id", run)).collect()
  }
}

/// One stored row, before it is checked.
struct StoredRow {
  id: String,
  task: String,
  run: Option<String>,
  author: String,
  kind: Option<String>,
  recipient: Option<String>,
  execution: Option<String>,
  invocation: Option<String>,
  body: String,
  created_at: i64,
  delivered_at: Option<i64>,
  acknowledged_at: Option<i64>,
  recovery_sequence: Option<i64>,
  timing: String,
  arrival: Option<String>,
  undelivered_execution: Option<String>,
  undelivered_at: Option<i64>,
  undelivered_reason: Option<String>,
  orchestrator: OrchestratorColumns,
  by_automation: bool,
}

impl StoredRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      task: row.get(1)?,
      run: row.get(2)?,
      author: row.get(3)?,
      kind: row.get(4)?,
      recipient: row.get(5)?,
      execution: row.get(6)?,
      invocation: row.get(7)?,
      body: row.get(8)?,
      created_at: row.get(9)?,
      delivered_at: row.get(10)?,
      acknowledged_at: row.get(11)?,
      recovery_sequence: row.get(12)?,
      timing: row.get(13)?,
      arrival: row.get(14)?,
      undelivered_execution: row.get(15)?,
      undelivered_at: row.get(16)?,
      undelivered_reason: row.get(17)?,
      orchestrator: OrchestratorColumns::read(row, (18, 20))?,
      by_automation: row.get(19)?,
    })
  }

  fn into_message(self) -> Result<Message, StoreError> {
    let sender = self.sender()?;
    let message = Message::restore(
      parse("message id", &self.id)?,
      parse("task id", &self.task)?,
      self.run.as_deref().map(|run| parse("run id", run)).transpose()?,
      sender,
      MessageBody::new(&self.body).map_err(|error| corrupt("message", error))?,
      Timestamp::from_unix_millis(self.created_at),
    )
    .map_err(|error| corrupt("message", error))?;
    let timing = match self.timing.as_str() {
      "after_turn" => Timing::AfterTurn,
      "now" => Timing::Now,
      other => return Err(corrupt("message timing", other)),
    };
    let message = if timing == Timing::AfterTurn {
      message
    } else {
      message.sent(timing).map_err(|error| corrupt("message", error))?
    };
    let message = if self.orchestrator.by {
      message
        .written_by_orchestrator(self.orchestrator.chat()?)
        .map_err(|error| corrupt("message", error))?
    } else {
      message
    };
    let message = if self.by_automation {
      message
        .sent_by_automation()
        .map_err(|error| corrupt("message", error))?
    } else {
      message
    };
    match self.recovery_sequence {
      None => Ok(message),
      Some(sequence) => {
        let sequence = u64::try_from(sequence).map_err(|error| corrupt("message recovery sequence", error))?;
        let sequence = NonZeroU64::new(sequence).ok_or_else(|| corrupt("message recovery sequence", "zero"))?;
        message
          .recovering_after(EventSequence::new(sequence))
          .map_err(|error| corrupt("message", error))
      }
    }
  }

  fn delivery(&self) -> Result<Delivery, StoreError> {
    let execution = || parse("step execution id", self.execution.as_deref().unwrap_or_default());
    let invocation = || parse("invocation id", self.invocation.as_deref().unwrap_or_default());
    let arrival = match self.arrival.as_deref() {
      None | Some("prompt") => Arrival::Prompt,
      Some("mid_turn") => Arrival::MidTurn,
      Some("turn_end") => Arrival::TurnEnd,
      Some(other) => return Err(corrupt("message arrival", other)),
    };
    if let (Some(undelivered), Some(at), Some(reason)) = (
      &self.undelivered_execution,
      self.undelivered_at,
      &self.undelivered_reason,
    ) {
      return Ok(Delivery::NotDelivered {
        execution: parse("step execution id", undelivered)?,
        at: Timestamp::from_unix_millis(at),
        reason: match reason.as_str() {
          "refused" => Undelivered::Refused,
          "unwritable" => Undelivered::Unwritable,
          other => return Err(corrupt("message undelivered reason", other)),
        },
      });
    }
    match (self.delivered_at, self.acknowledged_at) {
      (None, None) => Ok(Delivery::Queued),
      (Some(at), None) => Ok(Delivery::Delivered {
        execution: execution()?,
        invocation: invocation()?,
        at: Timestamp::from_unix_millis(at),
        arrival,
      }),
      (Some(delivered_at), Some(at)) => Ok(Delivery::Acknowledged {
        execution: execution()?,
        invocation: invocation()?,
        delivered_at: Timestamp::from_unix_millis(delivered_at),
        at: Timestamp::from_unix_millis(at),
        arrival,
      }),
      (None, Some(_)) => Err(corrupt("message delivery", "acknowledged but never delivered")),
    }
  }

  fn sender(&self) -> Result<Sender, StoreError> {
    let execution = || parse("step execution id", self.execution.as_deref().unwrap_or_default());
    let invocation = || parse("invocation id", self.invocation.as_deref().unwrap_or_default());
    match (self.author.as_str(), self.kind.as_deref()) {
      (PERSON, None) => {
        let recipient = match &self.recipient {
          None => Recipient::Orchestrator,
          Some(profile) => Recipient::Agent(parse("agent profile id", profile)?),
        };
        let delivery = self.delivery()?;
        Ok(Sender::Person { recipient, delivery })
      }
      (AGENT, Some(kind)) => Ok(Sender::Agent {
        execution: execution()?,
        invocation: invocation()?,
        kind: match kind {
          "reply" => AgentMessage::Reply,
          "question" => AgentMessage::Question,
          other => return Err(corrupt("agent message kind", other)),
        },
      }),
      (author, kind) => Err(corrupt("message author", format!("{author} / {kind:?}"))),
    }
  }
}
