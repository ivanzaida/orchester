//! What the runner records of its invocations, as values that can be sent (`docs/architecture/remote-runners.md`,
//! rule 2): an invocation's start, the entries of its transcript (`agent_events`), and each change to its rows —
//! `invocations` updated in place with its session, usage, end and outcome, what is kept with it once
//! (`invocation_prompts`, `invocation_accounts`, `invocation_connections`, `invocation_providers`,
//! `invocation_engine`, `invocation_code_index`, `invocation_memories`) and what is replaced
//! (`invocation_edit_progress`). [`ProjectDb`] writes each with the statements and in the transaction its direct write
//! uses. None holds a secret's value: a provider's key is named by its secret's id, and transcript events are
//! redacted before they become a record.

use std::sync::Arc;

use orchester_core::{
  code_index::CodeIndexUse,
  connection::{
    BaseUrl,
    stream::{AgentEvent, InvocationOutcome},
  },
  id::{AgentAccountId, ConnectionId, InvocationId, MemoryId, RunId, SecretId, StepExecutionId},
  memory::{Delivery, InjectedMemory, InjectionReason, MemoryStore},
  permission::LaunchPermissions,
  resource::ModelId,
  run::{EditProgress, Invocation, InvocationEngine, InvocationKind, InvocationPrompt, InvocationTiming, TokenUsage},
  store::ActivityStore,
  time::Timestamp,
  usage::UsedProvider,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{
  ProjectDb,
  activity::{AGENT, COMMAND, EVENT_SCHEMA, OUTCOME_SCHEMA, count_column, sequence_column},
  write_transaction::retried,
};
use crate::{
  StoreError, busy,
  rows::{corrupt, encode_json},
};

/// A new invocation of a step execution: the runner names it, the store numbers it after the execution's earlier
/// ones ([`ActivityStore::start_invocation`], [`ActivityStore::start_command_invocation`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationStart {
  id: InvocationId,
  run: RunId,
  execution: StepExecutionId,
  /// The agent CLI's model; `None` for a command tool's program.
  model: Option<ModelId>,
  at: Timestamp,
}

impl InvocationStart {
  /// A launch of the agent CLI on `model` for `execution` of `run`, at `at`.
  pub fn agent(run: RunId, execution: StepExecutionId, model: &ModelId, at: Timestamp) -> Self {
    Self::new(run, execution, Some(model.clone()), at)
  }

  /// A run of a tool step's command tool for `execution` of `run`, at `at`.
  pub fn command(run: RunId, execution: StepExecutionId, at: Timestamp) -> Self {
    Self::new(run, execution, None, at)
  }

  fn new(run: RunId, execution: StepExecutionId, model: Option<ModelId>, at: Timestamp) -> Self {
    Self {
      id: InvocationId::generate(),
      run,
      execution,
      model,
      at,
    }
  }

  /// The invocation as it started, numbered `number` by the store.
  pub fn invocation(&self, number: u32) -> Invocation {
    Invocation {
      id: self.id,
      run: self.run,
      execution: self.execution,
      number,
      kind: self.kind(),
      permissions: None,
      session: None,
      started_at: self.at,
      ended_at: None,
      usage: TokenUsage::default(),
      outcome: None,
      timing: InvocationTiming::default(),
    }
  }

  fn kind(&self) -> InvocationKind {
    self
      .model
      .clone()
      .map_or(InvocationKind::Command, |model| InvocationKind::Agent { model })
  }
}

/// Entries of an invocation's transcript, appended to its run's in one transaction
/// ([`ActivityStore::append_activity`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityRecord {
  run: RunId,
  execution: StepExecutionId,
  invocation: InvocationId,
  events: Vec<(Timestamp, AgentEvent)>,
}

impl ActivityRecord {
  /// `events` of `invocation`, already redacted.
  pub fn new(invocation: &Invocation, events: Vec<(Timestamp, AgentEvent)>) -> Self {
    Self {
      run: invocation.run,
      execution: invocation.execution,
      invocation: invocation.id,
      events,
    }
  }
}

/// A change to one invocation's rows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationRecord {
  invocation: InvocationId,
  change: InvocationChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum InvocationChange {
  State(InvocationState),
  Permissions { permissions: Box<LaunchPermissions> },
  Prompt { prompt: Box<InvocationPrompt> },
  CodeIndex { used: CodeIndexUse },
  EditProgress { progress: EditProgress },
  Memories { chosen: Vec<ChosenMemory> },
  Engine { engine: InvocationEngine },
  Connection { connection: ConnectionId, name: String },
  Account { account: AgentAccountId },
  Provider { base_url: BaseUrl, key: SecretId },
}

/// The columns of `invocations` [`ActivityStore::save_invocation`] updates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct InvocationState {
  session: Option<String>,
  ended_at: Option<Timestamp>,
  usage: TokenUsage,
  outcome: Option<InvocationOutcome>,
  inactive_ms: u64,
  waiting_since: Option<Timestamp>,
}

/// A record chosen for the invocation's prompt, as `invocation_memories` names its reason and delivery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ChosenMemory {
  memory: MemoryId,
  reason: String,
  tokens: u32,
  delivery: String,
}

impl InvocationRecord {
  const fn new(invocation: InvocationId, change: InvocationChange) -> Self {
    Self { invocation, change }
  }

  /// `invocation`'s session, usage, end, outcome and waits ([`ActivityStore::save_invocation`]).
  pub fn state(invocation: &Invocation) -> Self {
    Self::new(
      invocation.id,
      InvocationChange::State(InvocationState {
        session: invocation.session.clone(),
        ended_at: invocation.ended_at,
        usage: invocation.usage,
        outcome: invocation.outcome.clone(),
        inactive_ms: invocation.timing.inactive_ms(),
        waiting_since: invocation.timing.waiting_since(),
      }),
    )
  }

  /// The policy `invocation` was prepared with ([`ActivityStore::save_invocation_permissions`]).
  pub fn permissions(invocation: InvocationId, permissions: &LaunchPermissions) -> Self {
    let permissions = Box::new(permissions.clone());
    Self::new(invocation, InvocationChange::Permissions { permissions })
  }

  /// What `invocation` was told ([`ActivityStore::save_invocation_prompt`]).
  pub fn prompt(invocation: InvocationId, prompt: &InvocationPrompt) -> Self {
    let prompt = Box::new(prompt.clone());
    Self::new(invocation, InvocationChange::Prompt { prompt })
  }

  /// What `invocation` got of the code index ([`ActivityStore::save_invocation_code_index`]).
  pub fn code_index(invocation: InvocationId, used: &CodeIndexUse) -> Self {
    Self::new(invocation, InvocationChange::CodeIndex { used: used.clone() })
  }

  /// What `invocation` used before its first edit, replacing what was stored ([`ActivityStore::save_edit_progress`]).
  pub fn edit_progress(invocation: InvocationId, progress: &EditProgress) -> Self {
    let progress = progress.clone();
    Self::new(invocation, InvocationChange::EditProgress { progress })
  }

  /// The memory records chosen for `invocation`'s prompt ([`MemoryStore::save_injected_memory`]).
  pub fn memories(invocation: InvocationId, injected: &[InjectedMemory]) -> Self {
    let chosen = injected
      .iter()
      .map(|entry| ChosenMemory {
        memory: entry.memory,
        reason: entry.reason.name().to_owned(),
        tokens: entry.tokens,
        delivery: entry.delivery.name().to_owned(),
      })
      .collect();
    Self::new(invocation, InvocationChange::Memories { chosen })
  }

  /// What a native agent's `invocation` reaches its model through ([`ActivityStore::save_invocation_engine`]).
  pub fn engine(invocation: InvocationId, engine: &InvocationEngine) -> Self {
    Self::new(invocation, InvocationChange::Engine { engine: engine.clone() })
  }

  /// The connection `invocation` starts on ([`ProjectDb::save_invocation_connection`]).
  pub fn connection(invocation: InvocationId, connection: &UsedProvider) -> Self {
    let (connection, name) = (connection.id, connection.name.to_string());
    Self::new(invocation, InvocationChange::Connection { connection, name })
  }

  /// The account `invocation`'s CLI starts on ([`ProjectDb::save_invocation_account`]).
  pub const fn account(invocation: InvocationId, account: AgentAccountId) -> Self {
    Self::new(invocation, InvocationChange::Account { account })
  }

  /// The API provider `invocation`'s CLI starts on, with the secret holding its key
  /// ([`ProjectDb::save_invocation_provider`]).
  pub fn provider(invocation: InvocationId, base_url: &BaseUrl, key: SecretId) -> Self {
    let base_url = base_url.clone();
    Self::new(invocation, InvocationChange::Provider { base_url, key })
  }
}

impl ProjectDb {
  /// Records `start`, numbered after its execution's earlier invocations, in one transaction; its number.
  pub fn write_invocation_start(&mut self, start: &InvocationStart) -> Result<u32, StoreError> {
    let (kind, model) = match &start.model {
      Some(model) => (AGENT, model.as_str()),
      None => (COMMAND, ""),
    };
    retried(&mut self.connection, "start invocation", |transaction| {
      let previous: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(number), 0) FROM invocations WHERE step_execution_id = ?1",
        [start.execution.to_string()],
        |row| row.get(0),
      )?;
      let number = u32::try_from(previous + 1).map_err(|_| corrupt("invocation number", previous))?;
      transaction.execute(
        "INSERT INTO invocations (id, run_id, step_execution_id, number, kind, model, started_at, input_tokens, \
         cached_input_tokens, output_tokens) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, 0)",
        params![
          start.id.to_string(),
          start.run.to_string(),
          start.execution.to_string(),
          number,
          kind,
          model,
          start.at.unix_millis(),
        ],
      )?;
      Ok(number)
    })
  }

  /// Appends `record`'s entries to its run's transcript, in one transaction; the sequence of the last one (of the
  /// transcript so far when it has none).
  pub fn write_activity(&mut self, record: &ActivityRecord) -> Result<u64, StoreError> {
    append(
      &mut self.connection,
      (record.run, record.execution, record.invocation),
      &record.events,
    )
  }

  /// Applies `record`'s change to its invocation's rows, as its direct write does.
  pub fn write_invocation(&mut self, record: &InvocationRecord) -> Result<(), StoreError> {
    let invocation = record.invocation;
    match &record.change {
      InvocationChange::State(state) => state.update(&self.connection, invocation),
      InvocationChange::Permissions { permissions } => self.save_invocation_permissions(invocation, permissions),
      InvocationChange::Prompt { prompt } => self.save_invocation_prompt(invocation, prompt),
      InvocationChange::CodeIndex { used } => self.save_invocation_code_index(invocation, used),
      InvocationChange::EditProgress { progress } => self.save_edit_progress(invocation, progress),
      InvocationChange::Memories { chosen } => {
        let injected = chosen
          .iter()
          .map(ChosenMemory::injected)
          .collect::<Result<Vec<_>, _>>()?;
        self.save_injected_memory(invocation, &injected)
      }
      InvocationChange::Engine { engine } => self.save_invocation_engine(invocation, engine),
      InvocationChange::Connection { connection, name } => {
        let used = UsedProvider {
          id: *connection,
          name: Arc::from(name.as_str()),
        };
        self.save_invocation_connection(invocation, &used)
      }
      InvocationChange::Account { account } => self.save_invocation_account(invocation, *account),
      InvocationChange::Provider { base_url, key } => self.save_invocation_provider(invocation, base_url, *key),
    }
  }
}

impl ChosenMemory {
  fn injected(&self) -> Result<InjectedMemory, StoreError> {
    Ok(InjectedMemory {
      memory: self.memory,
      reason: InjectionReason::from_name(&self.reason).ok_or_else(|| corrupt("injection reason", &self.reason))?,
      tokens: self.tokens,
      delivery: Delivery::from_name(&self.delivery).ok_or_else(|| corrupt("memory delivery", &self.delivery))?,
    })
  }
}

impl InvocationState {
  /// Updates `invocation`'s row: one statement, run again while the database is busy. It stays outside a
  /// transaction, so what a trigger that fails it with `RAISE(FAIL)` changed before is kept, as it always was.
  fn update(&self, connection: &Connection, invocation: InvocationId) -> Result<(), StoreError> {
    let outcome = self
      .outcome
      .as_ref()
      .map(|outcome| encode_json("invocation outcome", OUTCOME_SCHEMA, outcome))
      .transpose()?;
    let changed = busy::retry("save invocation", || {
      connection
        .execute(
          "UPDATE invocations SET session_id = ?2, ended_at = ?3, input_tokens = ?4, cached_input_tokens = ?5, \
           output_tokens = ?6, outcome = ?7, inactive_ms = ?8, waiting_since = ?9 WHERE id = ?1",
          params![
            invocation.to_string(),
            self.session,
            self.ended_at.map(Timestamp::unix_millis),
            count_column(self.usage.input)?,
            count_column(self.usage.cached_input)?,
            count_column(self.usage.output)?,
            outcome,
            count_column(self.inactive_ms)?,
            self.waiting_since.map(Timestamp::unix_millis),
          ],
        )
        .map_err(StoreError::from)
    })?;
    if changed == 0 {
      return Err(StoreError::InvocationNotFound(invocation));
    }
    Ok(())
  }
}

/// Appends `events` of an invocation, named by its run, step execution and id, to its run's transcript.
pub(super) fn append(
  connection: &mut Connection,
  (run, execution, invocation): (RunId, StepExecutionId, InvocationId),
  events: &[(Timestamp, AgentEvent)],
) -> Result<u64, StoreError> {
  retried(connection, "append transcript", |transaction| {
    let mut last = last_sequence(transaction, run)?;
    let mut insert = transaction.prepare_cached(
      "INSERT INTO agent_events (run_id, sequence, step_execution_id, invocation_id, at, kind, payload) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for (at, event) in events {
      last += 1;
      insert.execute(params![
        run.to_string(),
        sequence_column(last)?,
        execution.to_string(),
        invocation.to_string(),
        at.unix_millis(),
        event.name(),
        encode_json("agent event", EVENT_SCHEMA, event)?,
      ])?;
    }
    Ok(last)
  })
}

fn last_sequence(connection: &Connection, run: RunId) -> Result<u64, StoreError> {
  let last: Option<i64> = connection
    .query_row(
      "SELECT MAX(sequence) FROM agent_events WHERE run_id = ?1",
      [run.to_string()],
      |row| row.get(0),
    )
    .optional()?
    .flatten();
  let last = last.unwrap_or(0);
  u64::try_from(last).map_err(|_| corrupt("agent event sequence", last))
}
