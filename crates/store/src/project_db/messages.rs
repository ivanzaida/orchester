//! Task conversations (`messages`): a person's messages, written by the desktop, and their delivery
//! and agents' messages, written by the runner. The schema keeps what a message says and lets its
//! delivery only move forward.

use orchester_core::{
  conversation::{AgentMessage, Delivery, Message, MessageBody, Recipient, Sender},
  id::{IdempotencyKey, TaskId},
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
                       created_at, delivered_at, acknowledged_at";

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
      let (execution, invocation, delivered_at, acknowledged_at) = delivery_columns(delivery);
      let updated = transaction.execute(
        "UPDATE messages SET run_id = ?2, step_execution_id = ?3, invocation_id = ?4, delivered_at = ?5, \
         acknowledged_at = ?6 WHERE id = ?1",
        params![
          message.id().to_string(),
          message.run().map(|run| run.to_string()),
          execution,
          invocation,
          delivered_at,
          acknowledged_at,
        ],
      )?;
      if updated != 1 {
        return Err(corrupt("message", format!("{} is not stored", message.id())));
      }
    }
    transaction.commit()?;
    Ok(())
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
}

fn insert(connection: &Connection, message: &Message, key: Option<IdempotencyKey>) -> Result<(), StoreError> {
  let (author, kind, recipient, execution, invocation, delivered_at, acknowledged_at) = match *message.sender() {
    Sender::Person { recipient, delivery } => {
      let recipient = match recipient {
        Recipient::Orchestrator => None,
        Recipient::Agent(profile) => Some(profile.to_string()),
      };
      let (execution, invocation, delivered_at, acknowledged_at) = delivery_columns(delivery);
      (
        PERSON,
        None,
        recipient,
        execution,
        invocation,
        delivered_at,
        acknowledged_at,
      )
    }
    Sender::Agent {
      execution,
      invocation,
      kind,
    } => (
      AGENT,
      Some(kind_text(kind)),
      None,
      Some(execution.to_string()),
      Some(invocation.to_string()),
      None,
      None,
    ),
  };
  connection.execute(
    "INSERT INTO messages (id, task_id, run_id, idempotency_key, author, kind, recipient_profile, step_execution_id, \
     invocation_id, body, created_at, delivered_at, acknowledged_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    params![
      message.id().to_string(),
      message.task().to_string(),
      message.run().map(|run| run.to_string()),
      key.map(|key| key.to_string()),
      author,
      kind,
      recipient,
      execution,
      invocation,
      message.body().as_str(),
      message.created_at().unix_millis(),
      delivered_at,
      acknowledged_at,
    ],
  )?;
  Ok(())
}

type DeliveryColumns = (Option<String>, Option<String>, Option<i64>, Option<i64>);

fn delivery_columns(delivery: Delivery) -> DeliveryColumns {
  match delivery {
    Delivery::Queued => (None, None, None, None),
    Delivery::Delivered {
      execution,
      invocation,
      at,
    } => (
      Some(execution.to_string()),
      Some(invocation.to_string()),
      Some(at.unix_millis()),
      None,
    ),
    Delivery::Acknowledged {
      execution,
      invocation,
      delivered_at,
      at,
    } => (
      Some(execution.to_string()),
      Some(invocation.to_string()),
      Some(delivered_at.unix_millis()),
      Some(at.unix_millis()),
    ),
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
    })
  }

  fn into_message(self) -> Result<Message, StoreError> {
    let sender = self.sender()?;
    Message::restore(
      parse("message id", &self.id)?,
      parse("task id", &self.task)?,
      self.run.as_deref().map(|run| parse("run id", run)).transpose()?,
      sender,
      MessageBody::new(&self.body).map_err(|error| corrupt("message", error))?,
      Timestamp::from_unix_millis(self.created_at),
    )
    .map_err(|error| corrupt("message", error))
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
        let delivery = match (self.delivered_at, self.acknowledged_at) {
          (None, None) => Delivery::Queued,
          (Some(at), None) => Delivery::Delivered {
            execution: execution()?,
            invocation: invocation()?,
            at: Timestamp::from_unix_millis(at),
          },
          (Some(delivered_at), Some(at)) => Delivery::Acknowledged {
            execution: execution()?,
            invocation: invocation()?,
            delivered_at: Timestamp::from_unix_millis(delivered_at),
            at: Timestamp::from_unix_millis(at),
          },
          (None, Some(_)) => return Err(corrupt("message delivery", "acknowledged but never delivered")),
        };
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
