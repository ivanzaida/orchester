//! What the runner records of a task's conversation, as values that can be sent (`docs/architecture/remote-runners.md`,
//! rule 2): a new message, and how far people's messages got — several delivered or acknowledged together, or one
//! delivery that another writer may have moved on first. Each record holds the columns its write binds, and
//! [`ProjectDb::write_message`] writes it with the statement and in the transaction the conversation's direct writes
//! ([`orchester_core::store::ConversationStore`]) use, as they go through it too; the schema still lets a delivery
//! only move forward.

use orchester_core::{
  conversation::{AgentMessage, Arrival, Delivery, Message, Recipient, Sender, Timing, Undelivered},
  id::{ChatId, IdempotencyKey, InvocationId, MessageId, ResourceId, RunId, StepExecutionId, TaskId},
  run::Answerer,
  time::Timestamp,
};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use super::{
  ProjectDb,
  messages::{AGENT, PERSON},
};
use crate::{StoreError, rows::corrupt};

/// A change to a task's conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRecord(MessageChange);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum MessageChange {
  Added {
    row: Box<MessageRow>,
  },
  Deliveries {
    deliveries: Vec<DeliveryRow>,
  },
  Delivery {
    before: Standing,
    changed: Box<DeliveryRow>,
    by_automation: bool,
  },
}

/// The columns of a new `messages` row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct MessageRow {
  id: MessageId,
  task: TaskId,
  run: Option<RunId>,
  key: Option<IdempotencyKey>,
  author: String,
  kind: Option<String>,
  recipient: Option<ResourceId>,
  delivery: DeliveryColumns,
  body: String,
  created_at: Timestamp,
  recovery_sequence: Option<u64>,
  timing: String,
  by_orchestrator: bool,
  by_automation: bool,
  orchestrator_chat: Option<ChatId>,
}

/// How far one person's message got, as its row stores it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct DeliveryRow {
  message: MessageId,
  run: Option<RunId>,
  columns: DeliveryColumns,
  timing: String,
}

/// Where a message stood before a change that two writers can race to make: the change applies only while it still
/// does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Standing {
  delivered_at: Option<Timestamp>,
  acknowledged_at: Option<Timestamp>,
  undelivered_at: Option<Timestamp>,
  timing: String,
}

/// How a delivery is stored.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct DeliveryColumns {
  execution: Option<StepExecutionId>,
  invocation: Option<InvocationId>,
  delivered_at: Option<Timestamp>,
  acknowledged_at: Option<Timestamp>,
  arrival: Option<String>,
  undelivered_execution: Option<StepExecutionId>,
  undelivered_at: Option<Timestamp>,
  undelivered_reason: Option<String>,
}

impl MessageRecord {
  /// `message`, new in its task's conversation ([`orchester_core::store::ConversationStore::add_agent_message`]).
  pub fn added(message: &Message) -> Self {
    let row = Box::new(MessageRow::of(message, None));
    Self(MessageChange::Added { row })
  }

  /// How far each of people's `messages` got, written together
  /// ([`orchester_core::store::ConversationStore::save_deliveries`]); an agent's message has no delivery.
  pub fn deliveries(messages: &[Message]) -> Result<Self, StoreError> {
    let deliveries = messages.iter().map(DeliveryRow::of).collect::<Result<_, _>>()?;
    Ok(Self(MessageChange::Deliveries { deliveries }))
  }

  /// How far a person's message got, `changed`, only while it still stands as `before`
  /// ([`orchester_core::store::ConversationStore::change_delivery`]).
  pub fn delivery_change(before: &Message, changed: &Message) -> Result<Self, StoreError> {
    let Some(was) = before.delivery() else {
      return Err(StoreError::Unrepresentable("an agent's message as delivered"));
    };
    let was = DeliveryColumns::of(was);
    let before = Standing {
      delivered_at: was.delivered_at,
      acknowledged_at: was.acknowledged_at,
      undelivered_at: was.undelivered_at,
      timing: timing_text(before.timing()).to_owned(),
    };
    Ok(Self(MessageChange::Delivery {
      before,
      changed: Box::new(DeliveryRow::of(changed)?),
      // Automation that sends a message again is recorded as its sender, as for one it wrote.
      by_automation: changed.answerer() == Some(Answerer::Automation),
    }))
  }
}

impl ProjectDb {
  /// Applies `record` to its task's conversation, as its direct write does: whether it changed what it names, which
  /// only a delivery change another writer got to first does not.
  pub fn write_message(&mut self, record: &MessageRecord) -> Result<bool, StoreError> {
    match &record.0 {
      MessageChange::Added { row } => row.insert(&self.connection).map(|()| true),
      MessageChange::Deliveries { deliveries } => {
        let transaction = self.connection.transaction()?;
        for delivery in deliveries {
          delivery.save(&transaction)?;
        }
        transaction.commit()?;
        Ok(true)
      }
      MessageChange::Delivery {
        before,
        changed,
        by_automation,
      } => changed.change(&self.connection, before, *by_automation),
    }
  }
}

/// Stores `message`, a person's one once per `key`.
pub(super) fn insert(
  connection: &Connection,
  message: &Message,
  key: Option<IdempotencyKey>,
) -> Result<(), StoreError> {
  MessageRow::of(message, key).insert(connection)
}

impl MessageRow {
  fn of(message: &Message, key: Option<IdempotencyKey>) -> Self {
    let (author, kind, recipient, delivery) = match *message.sender() {
      Sender::Person { recipient, delivery } => {
        let recipient = match recipient {
          Recipient::Orchestrator => None,
          Recipient::Agent(profile) => Some(profile),
        };
        (PERSON, None, recipient, DeliveryColumns::of(delivery))
      }
      Sender::Agent {
        execution,
        invocation,
        kind,
      } => (
        AGENT,
        Some(kind_text(kind).to_owned()),
        None,
        DeliveryColumns {
          execution: Some(execution),
          invocation: Some(invocation),
          ..DeliveryColumns::default()
        },
      ),
    };
    Self {
      id: message.id(),
      task: message.task(),
      run: message.run(),
      key,
      author: author.to_owned(),
      kind,
      recipient,
      delivery,
      body: message.body().as_str().to_owned(),
      created_at: message.created_at(),
      recovery_sequence: message.recovery_after().map(|sequence| sequence.get()),
      timing: timing_text(message.timing()).to_owned(),
      by_orchestrator: message.by_orchestrator(),
      by_automation: message.answerer() == Some(Answerer::Automation),
      orchestrator_chat: message.from_orchestrator().and_then(|from| from.chat),
    }
  }

  fn insert(&self, connection: &Connection) -> Result<(), StoreError> {
    let columns = &self.delivery;
    connection.execute(
      "INSERT INTO messages (id, task_id, run_id, idempotency_key, author, kind, recipient_profile, step_execution_id, \
       invocation_id, body, created_at, delivered_at, acknowledged_at, recovery_sequence, timing, arrival, \
       undelivered_execution, undelivered_at, undelivered_reason, by_orchestrator, by_automation, orchestrator_chat) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
      params![
        self.id.to_string(),
        self.task.to_string(),
        self.run.map(|run| run.to_string()),
        self.key.map(|key| key.to_string()),
        self.author,
        self.kind,
        self.recipient.map(|profile| profile.to_string()),
        columns.execution.map(|execution| execution.to_string()),
        columns.invocation.map(|invocation| invocation.to_string()),
        self.body,
        self.created_at.unix_millis(),
        columns.delivered_at.map(Timestamp::unix_millis),
        columns.acknowledged_at.map(Timestamp::unix_millis),
        self
          .recovery_sequence
          .map(i64::try_from)
          .transpose()
          .map_err(|error| corrupt("message recovery sequence", error))?,
        self.timing,
        columns.arrival,
        columns.undelivered_execution.map(|execution| execution.to_string()),
        columns.undelivered_at.map(Timestamp::unix_millis),
        columns.undelivered_reason,
        self.by_orchestrator,
        self.by_automation,
        self.orchestrator_chat.map(|chat| chat.to_string()),
      ],
    )?;
    Ok(())
  }
}

impl DeliveryRow {
  fn of(message: &Message) -> Result<Self, StoreError> {
    let Some(delivery) = message.delivery() else {
      return Err(StoreError::Unrepresentable("an agent's message as delivered"));
    };
    Ok(Self {
      message: message.id(),
      run: message.run(),
      columns: DeliveryColumns::of(delivery),
      timing: timing_text(message.timing()).to_owned(),
    })
  }

  /// Stores the delivery; the message must be stored.
  fn save(&self, connection: &Connection) -> Result<(), StoreError> {
    let columns = &self.columns;
    let updated = connection.execute(
      "UPDATE messages SET run_id = ?2, step_execution_id = ?3, invocation_id = ?4, delivered_at = ?5, \
       acknowledged_at = ?6, arrival = ?7, undelivered_execution = ?8, undelivered_at = ?9, \
       undelivered_reason = ?10, timing = ?11 WHERE id = ?1",
      params![
        self.message.to_string(),
        self.run.map(|run| run.to_string()),
        columns.execution.map(|execution| execution.to_string()),
        columns.invocation.map(|invocation| invocation.to_string()),
        columns.delivered_at.map(Timestamp::unix_millis),
        columns.acknowledged_at.map(Timestamp::unix_millis),
        columns.arrival,
        columns.undelivered_execution.map(|execution| execution.to_string()),
        columns.undelivered_at.map(Timestamp::unix_millis),
        columns.undelivered_reason,
        self.timing,
      ],
    )?;
    if updated != 1 {
      return Err(corrupt("message", format!("{} is not stored", self.message)));
    }
    Ok(())
  }

  /// Stores the delivery while the message still stands as `before`; whether it did.
  fn change(&self, connection: &Connection, before: &Standing, by_automation: bool) -> Result<bool, StoreError> {
    let columns = &self.columns;
    let updated = connection.execute(
      "UPDATE messages SET run_id = ?2, step_execution_id = ?3, invocation_id = ?4, delivered_at = ?5, \
       acknowledged_at = ?6, arrival = ?7, undelivered_execution = ?8, undelivered_at = ?9, \
       undelivered_reason = ?10, timing = ?11, by_automation = by_automation OR ?16 WHERE id = ?1 \
       AND delivered_at IS ?12 AND acknowledged_at IS ?13 AND undelivered_at IS ?14 AND timing = ?15",
      params![
        self.message.to_string(),
        self.run.map(|run| run.to_string()),
        columns.execution.map(|execution| execution.to_string()),
        columns.invocation.map(|invocation| invocation.to_string()),
        columns.delivered_at.map(Timestamp::unix_millis),
        columns.acknowledged_at.map(Timestamp::unix_millis),
        columns.arrival,
        columns.undelivered_execution.map(|execution| execution.to_string()),
        columns.undelivered_at.map(Timestamp::unix_millis),
        columns.undelivered_reason,
        self.timing,
        before.delivered_at.map(Timestamp::unix_millis),
        before.acknowledged_at.map(Timestamp::unix_millis),
        before.undelivered_at.map(Timestamp::unix_millis),
        before.timing,
        by_automation,
      ],
    )?;
    Ok(updated == 1)
  }
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
        execution: Some(execution),
        invocation: Some(invocation),
        delivered_at: Some(at),
        arrival: Some(arrival_text(arrival).to_owned()),
        ..Self::default()
      },
      Delivery::Acknowledged {
        execution,
        invocation,
        delivered_at,
        at,
        arrival,
      } => Self {
        execution: Some(execution),
        invocation: Some(invocation),
        delivered_at: Some(delivered_at),
        acknowledged_at: Some(at),
        arrival: Some(arrival_text(arrival).to_owned()),
        ..Self::default()
      },
      Delivery::NotDelivered { execution, at, reason } => Self {
        undelivered_execution: Some(execution),
        undelivered_at: Some(at),
        undelivered_reason: Some(
          match reason {
            Undelivered::Refused => "refused",
            Undelivered::Unwritable => "unwritable",
          }
          .to_owned(),
        ),
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
