//! Task conversations (`messages`): a person's messages, written by the desktop, and their delivery
//! and agents' messages, written by the runner. The schema keeps what a message says and lets its
//! delivery only move forward; a message sent now that was not delivered is queued again only when
//! the person sends it after the turn.

use std::num::NonZeroU64;

use orchester_core::{
  conversation::{AgentMessage, Arrival, Delivery, Message, MessageBody, Recipient, Sender, Timing, Undelivered},
  id::{IdempotencyKey, RunId, TaskId},
  run::EventSequence,
  store::ConversationStore,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

const PERSON: &str = "person";
const AGENT: &str = "agent";
const COLUMNS: &str = "id, task_id, run_id, author, kind, recipient_profile, step_execution_id, invocation_id, body, \
                       created_at, delivered_at, acknowledged_at, recovery_sequence, timing, arrival, \
                       undelivered_execution, undelivered_at, undelivered_reason, by_orchestrator";

impl ConversationStore for ProjectDb {
  type Error = StoreError;

  fn send_message(&mut self, key: IdempotencyKey, message: &Message) -> Result<Message, StoreError> {
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
    let task_exists: bool = transaction.query_row(
      "SELECT EXISTS (SELECT 1 FROM tasks WHERE id = ?1)",
      [message.task().to_string()],
      |row| row.get(0),
    )?;
    if !task_exists {
      return Err(StoreError::TaskNotFound(message.task()));
    }
    insert(&transaction, message, Some(key))?;
    transaction.commit()?;
    Ok(message.clone())
  }

  fn add_agent_message(&mut self, message: &Message) -> Result<(), StoreError> {
    insert(&self.connection, message, None)
  }

  fn save_deliveries(&mut self, messages: &[Message]) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    for message in messages {
      let Some(delivery) = message.delivery() else {
        return Err(StoreError::Unrepresentable("an agent's message as delivered"));
      };
      let columns = DeliveryColumns::of(delivery);
      let updated = transaction.execute(
        "UPDATE messages SET run_id = ?2, step_execution_id = ?3, invocation_id = ?4, delivered_at = ?5, \
         acknowledged_at = ?6, arrival = ?7, undelivered_execution = ?8, undelivered_at = ?9, \
         undelivered_reason = ?10, timing = ?11 WHERE id = ?1",
        params![
          message.id().to_string(),
          message.run().map(|run| run.to_string()),
          columns.execution,
          columns.invocation,
          columns.delivered_at,
          columns.acknowledged_at,
          columns.arrival,
          columns.undelivered_execution,
          columns.undelivered_at,
          columns.undelivered_reason,
          timing_text(message.timing()),
        ],
      )?;
      if updated != 1 {
        return Err(corrupt("message", format!("{} is not stored", message.id())));
      }
    }
    transaction.commit()?;
    Ok(())
  }

  fn change_delivery(&mut self, before: &Message, changed: &Message) -> Result<bool, StoreError> {
    let (Some(was), Some(delivery)) = (before.delivery(), changed.delivery()) else {
      return Err(StoreError::Unrepresentable("an agent's message as delivered"));
    };
    let (was, columns) = (DeliveryColumns::of(was), DeliveryColumns::of(delivery));
    let updated = self.connection.execute(
      "UPDATE messages SET run_id = ?2, step_execution_id = ?3, invocation_id = ?4, delivered_at = ?5, \
       acknowledged_at = ?6, arrival = ?7, undelivered_execution = ?8, undelivered_at = ?9, \
       undelivered_reason = ?10, timing = ?11 WHERE id = ?1 AND delivered_at IS ?12 AND acknowledged_at IS ?13 \
       AND undelivered_at IS ?14 AND timing = ?15",
      params![
        changed.id().to_string(),
        changed.run().map(|run| run.to_string()),
        columns.execution,
        columns.invocation,
        columns.delivered_at,
        columns.acknowledged_at,
        columns.arrival,
        columns.undelivered_execution,
        columns.undelivered_at,
        columns.undelivered_reason,
        timing_text(changed.timing()),
        was.delivered_at,
        was.acknowledged_at,
        was.undelivered_at,
        timing_text(before.timing()),
      ],
    )?;
    Ok(updated == 1)
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

fn insert(connection: &Connection, message: &Message, key: Option<IdempotencyKey>) -> Result<(), StoreError> {
  let (author, kind, recipient, columns) = match *message.sender() {
    Sender::Person { recipient, delivery } => {
      let recipient = match recipient {
        Recipient::Orchestrator => None,
        Recipient::Agent(profile) => Some(profile.to_string()),
      };
      (PERSON, None, recipient, DeliveryColumns::of(delivery))
    }
    Sender::Agent {
      execution,
      invocation,
      kind,
    } => (
      AGENT,
      Some(kind_text(kind)),
      None,
      DeliveryColumns {
        execution: Some(execution.to_string()),
        invocation: Some(invocation.to_string()),
        ..DeliveryColumns::default()
      },
    ),
  };
  connection.execute(
    "INSERT INTO messages (id, task_id, run_id, idempotency_key, author, kind, recipient_profile, step_execution_id, \
     invocation_id, body, created_at, delivered_at, acknowledged_at, recovery_sequence, timing, arrival, \
     undelivered_execution, undelivered_at, undelivered_reason, by_orchestrator) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
    params![
      message.id().to_string(),
      message.task().to_string(),
      message.run().map(|run| run.to_string()),
      key.map(|key| key.to_string()),
      author,
      kind,
      recipient,
      columns.execution,
      columns.invocation,
      message.body().as_str(),
      message.created_at().unix_millis(),
      columns.delivered_at,
      columns.acknowledged_at,
      message
        .recovery_after()
        .map(|sequence| i64::try_from(sequence.get()))
        .transpose()
        .map_err(|error| corrupt("message recovery sequence", error))?,
      timing_text(message.timing()),
      columns.arrival,
      columns.undelivered_execution,
      columns.undelivered_at,
      columns.undelivered_reason,
      message.by_orchestrator(),
    ],
  )?;
  Ok(())
}

/// How a delivery is stored.
#[derive(Default)]
struct DeliveryColumns {
  execution: Option<String>,
  invocation: Option<String>,
  delivered_at: Option<i64>,
  acknowledged_at: Option<i64>,
  arrival: Option<&'static str>,
  undelivered_execution: Option<String>,
  undelivered_at: Option<i64>,
  undelivered_reason: Option<&'static str>,
}

impl DeliveryColumns {
  fn of(delivery: Delivery) -> Self {
    match delivery {
      Delivery::Queued => Self::default(),
      Delivery::Delivered {
        execution,
        invocation,
        at,
        arrival,
      } => Self {
        execution: Some(execution.to_string()),
        invocation: Some(invocation.to_string()),
        delivered_at: Some(at.unix_millis()),
        arrival: Some(arrival_text(arrival)),
        ..Self::default()
      },
      Delivery::Acknowledged {
        execution,
        invocation,
        delivered_at,
        at,
        arrival,
      } => Self {
        execution: Some(execution.to_string()),
        invocation: Some(invocation.to_string()),
        delivered_at: Some(delivered_at.unix_millis()),
        acknowledged_at: Some(at.unix_millis()),
        arrival: Some(arrival_text(arrival)),
        ..Self::default()
      },
      Delivery::NotDelivered { execution, at, reason } => Self {
        undelivered_execution: Some(execution.to_string()),
        undelivered_at: Some(at.unix_millis()),
        undelivered_reason: Some(match reason {
          Undelivered::Refused => "refused",
          Undelivered::Unwritable => "unwritable",
        }),
        ..Self::default()
      },
    }
  }
}

const fn timing_text(timing: Timing) -> &'static str {
  match timing {
    Timing::AfterTurn => "after_turn",
    Timing::Now => "now",
  }
}

const fn arrival_text(arrival: Arrival) -> &'static str {
  match arrival {
    Arrival::Prompt => "prompt",
    Arrival::MidTurn => "mid_turn",
    Arrival::TurnEnd => "turn_end",
  }
}

const fn kind_text(kind: AgentMessage) -> &'static str {
  match kind {
    AgentMessage::Reply => "reply",
    AgentMessage::Question => "question",
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
  by_orchestrator: bool,
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
      by_orchestrator: row.get(18)?,
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
    let message = if self.by_orchestrator {
      message.sent_by_orchestrator().map_err(|error| corrupt("message", error))?
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
