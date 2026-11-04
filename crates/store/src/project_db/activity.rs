//! Agent CLI and command tool invocations and the transcript of their events, written by the runner. The writes go
//! through the records the runner's executor sends ([`super::invocation_records`]).

use orchester_core::{
  code_index::CodeIndexUse,
  connection::stream::{AgentEvent, InvocationOutcome},
  id::{InvocationId, RunId, StepExecutionId},
  permission::{AgentPermissions, LaunchPermissions},
  resource::ModelId,
  run::{
    ActivityEntry, EditProgress, Invocation, InvocationEngine, InvocationKind, InvocationPrompt, InvocationTiming,
    StepOutput, TokenUsage,
  },
  store::ActivityStore,
  time::Timestamp,
  tokens::TokensUsed,
};
use rusqlite::{OptionalExtension, Row, params};

use super::{
  ProjectDb, code_index, estimates, invocation_prompts,
  invocation_records::{InvocationRecord, InvocationStart, append},
  native, orchestrator_usage, reviewer_usage, step_outputs,
  write_transaction::retried,
};
use crate::{
  StoreError,
  rows::{corrupt, decode_body, decode_document, decode_json, encode_json, parse},
};

pub(super) const EVENT_SCHEMA: u32 = 1;
pub(super) const OUTCOME_SCHEMA: u32 = 1;
/// 2: launch permissions with what happened to commands outside them; 1 held the permissions alone.
const PERMISSIONS_SCHEMA: u32 = 2;

const SELECT_INVOCATIONS: &str = "SELECT id, run_id, step_execution_id, number, kind, model, session_id, \
                                  started_at, ended_at, input_tokens, cached_input_tokens, output_tokens, outcome, \
                                  inactive_ms, waiting_since, permissions \
                                  FROM invocations";
/// `kind` of an agent CLI's invocation and of a command tool's.
pub(super) const AGENT: &str = "agent";
pub(super) const COMMAND: &str = "command";

impl ActivityStore for ProjectDb {
  type Error = StoreError;

  fn start_invocation(
    &mut self,
    run: RunId,
    execution: StepExecutionId,
    model: &ModelId,
    now: Timestamp,
  ) -> Result<Invocation, StoreError> {
    let start = InvocationStart::agent(run, execution, model, now);
    let number = self.write_invocation_start(&start)?;
    Ok(start.invocation(number))
  }

  fn start_command_invocation(
    &mut self,
    run: RunId,
    execution: StepExecutionId,
    now: Timestamp,
  ) -> Result<Invocation, StoreError> {
    let start = InvocationStart::command(run, execution, now);
    let number = self.write_invocation_start(&start)?;
    Ok(start.invocation(number))
  }

  fn save_invocation(&mut self, invocation: &Invocation) -> Result<(), StoreError> {
    self.write_invocation(&InvocationRecord::state(invocation))
  }

  fn save_invocation_permissions(
    &mut self,
    invocation: InvocationId,
    permissions: &LaunchPermissions,
  ) -> Result<(), StoreError> {
    let changed = self.connection.execute(
      "UPDATE invocations SET permissions = ?2 \
       WHERE id = ?1 AND kind = 'agent' AND ended_at IS NULL AND permissions IS NULL",
      params![
        invocation.to_string(),
        encode_json("invocation permissions", PERMISSIONS_SCHEMA, permissions)?,
      ],
    )?;
    if changed == 0 {
      let exists: bool = self.connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM invocations WHERE id = ?1)",
        [invocation.to_string()],
        |row| row.get(0),
      )?;
      return Err(if exists {
        StoreError::InvocationPermissionsNotRecordable(invocation)
      } else {
        StoreError::InvocationNotFound(invocation)
      });
    }
    Ok(())
  }

  fn invocations(&self, run: RunId) -> Result<Vec<Invocation>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "{SELECT_INVOCATIONS} WHERE run_id = ?1 ORDER BY started_at, rowid"
    ))?;
    let rows = statement
      .query_map([run.to_string()], InvocationRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(InvocationRow::into_invocation).collect()
  }

  fn save_invocation_prompt(&mut self, invocation: InvocationId, prompt: &InvocationPrompt) -> Result<(), StoreError> {
    invocation_prompts::save(&self.connection, invocation, prompt)
  }

  fn invocation_prompt(&self, invocation: InvocationId) -> Result<Option<InvocationPrompt>, StoreError> {
    invocation_prompts::of_invocation(&self.connection, invocation)
  }

  fn save_invocation_code_index(&mut self, invocation: InvocationId, used: &CodeIndexUse) -> Result<(), StoreError> {
    code_index::save_use(&self.connection, invocation, used)
  }

  fn invocation_code_index(&self, invocation: InvocationId) -> Result<Option<CodeIndexUse>, StoreError> {
    code_index::load_use(&self.connection, invocation)
  }

  fn save_invocation_engine(&mut self, invocation: InvocationId, engine: &InvocationEngine) -> Result<(), StoreError> {
    native::save_engine(&self.connection, invocation, engine)
  }

  fn invocation_engine(&self, invocation: InvocationId) -> Result<Option<InvocationEngine>, StoreError> {
    native::load_engine(&self.connection, invocation)
  }

  fn save_edit_progress(&mut self, invocation: InvocationId, progress: &EditProgress) -> Result<(), StoreError> {
    code_index::save_progress(&self.connection, invocation, progress)
  }

  fn edit_progress(&self, invocation: InvocationId) -> Result<Option<EditProgress>, StoreError> {
    code_index::load_progress(&self.connection, invocation)
  }

  fn append_activity(
    &mut self,
    invocation: &Invocation,
    events: &[(Timestamp, AgentEvent)],
  ) -> Result<u64, StoreError> {
    append(
      &mut self.connection,
      (invocation.run, invocation.execution, invocation.id),
      events,
    )
  }

  fn last_agent_message(&self, run: RunId, execution: StepExecutionId) -> Result<Option<(u64, String)>, StoreError> {
    let row: Option<(i64, String)> = self
      .connection
      .query_row(
        "
        SELECT sequence, payload FROM agent_events
        WHERE run_id = ?1 AND step_execution_id = ?2 AND kind = 'message'
        ORDER BY sequence DESC LIMIT 1",
        params![run.to_string(), execution.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    let Some((sequence, payload)) = row else {
      return Ok(None);
    };
    let sequence = u64::try_from(sequence).map_err(|_| corrupt("agent event sequence", sequence))?;
    match decode_json("agent event", EVENT_SCHEMA, &payload)? {
      AgentEvent::Message { text } => Ok(Some((sequence, text))),
      other => Err(corrupt(
        "agent event",
        format!("kind `message` holds a `{}` payload", other.name()),
      )),
    }
  }

  fn last_command_finished(&self, run: RunId, execution: StepExecutionId) -> Result<Option<AgentEvent>, StoreError> {
    let payload: Option<String> = self
      .connection
      .query_row(
        "
        SELECT payload FROM agent_events
        WHERE run_id = ?1 AND step_execution_id = ?2 AND kind = 'tool_command_finished'
        ORDER BY sequence DESC LIMIT 1",
        params![run.to_string(), execution.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    let Some(payload) = payload else {
      return Ok(None);
    };
    match decode_json("agent event", EVENT_SCHEMA, &payload)? {
      finished @ AgentEvent::ToolCommandFinished { .. } => Ok(Some(finished)),
      other => Err(corrupt(
        "agent event",
        format!("kind `tool_command_finished` holds a `{}` payload", other.name()),
      )),
    }
  }

  fn activity(&self, run: RunId, after: u64) -> Result<Vec<ActivityEntry>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT sequence, step_execution_id, invocation_id, at, kind, payload FROM agent_events \
       WHERE run_id = ?1 AND sequence > ?2 ORDER BY sequence",
    )?;
    let rows = statement
      .query_map(params![run.to_string(), sequence_column(after)?], |row| {
        Ok((
          row.get::<_, i64>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, i64>(3)?,
          row.get::<_, String>(4)?,
          row.get::<_, String>(5)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(sequence, execution, invocation, at, kind, payload)| {
        let event: AgentEvent = decode_json("agent event", EVENT_SCHEMA, &payload)?;
        if event.name() != kind {
          return Err(corrupt(
            "agent event",
            format!("kind `{kind}` holds a `{}` payload", event.name()),
          ));
        }
        Ok(ActivityEntry {
          sequence: u64::try_from(sequence).map_err(|_| corrupt("agent event sequence", sequence))?,
          execution: parse("step execution id", &execution)?,
          invocation: parse("invocation id", &invocation)?,
          at: Timestamp::from_unix_millis(at),
          event,
        })
      })
      .collect()
  }

  fn tokens_used_since(&self, since: Timestamp) -> Result<TokensUsed, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT payload FROM agent_events WHERE kind = 'turn_completed' AND at >= ?1")?;
    let payloads = statement
      .query_map([since.unix_millis()], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    let mut used = reviewer_usage::counted_since(&self.connection, since)?;
    used += orchestrator_usage::counted_since(&self.connection, since)?;
    used += estimates::counted_since(&self.connection, since)?;
    for payload in payloads {
      match decode_json("agent event", EVENT_SCHEMA, &payload)? {
        AgentEvent::TurnCompleted { usage } => used += usage.counted_tokens(),
        other => {
          return Err(corrupt(
            "agent event",
            format!("a `turn_completed` row holds `{}`", other.name()),
          ));
        }
      }
    }
    Ok(used)
  }

  fn save_step_output(&mut self, output: &StepOutput) -> Result<(), StoreError> {
    self.write_step(&super::step_records::StepRecord::output(output)?)
  }

  fn step_outputs(&self, run: RunId) -> Result<Vec<StepOutput>, StoreError> {
    step_outputs::of_run(&self.connection, run)
  }
}

pub(super) fn sequence_column(sequence: u64) -> Result<i64, StoreError> {
  i64::try_from(sequence).map_err(|_| StoreError::Unrepresentable("agent event sequence"))
}

pub(super) fn count_column(count: u64) -> Result<i64, StoreError> {
  i64::try_from(count).map_err(|_| StoreError::Unrepresentable("token count"))
}

struct InvocationRow {
  id: String,
  run: String,
  execution: String,
  number: i64,
  kind: String,
  model: String,
  session: Option<String>,
  started_at: i64,
  ended_at: Option<i64>,
  tokens: [i64; 3],
  outcome: Option<String>,
  inactive_ms: i64,
  waiting_since: Option<i64>,
  permissions: Option<String>,
}

impl InvocationRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      run: row.get(1)?,
      execution: row.get(2)?,
      number: row.get(3)?,
      kind: row.get(4)?,
      model: row.get(5)?,
      session: row.get(6)?,
      started_at: row.get(7)?,
      ended_at: row.get(8)?,
      tokens: [row.get(9)?, row.get(10)?, row.get(11)?],
      outcome: row.get(12)?,
      inactive_ms: row.get(13)?,
      waiting_since: row.get(14)?,
      permissions: row.get(15)?,
    })
  }

  fn into_invocation(self) -> Result<Invocation, StoreError> {
    let count = |value: i64| u64::try_from(value).map_err(|_| corrupt("token count", value));
    let outcome: Option<InvocationOutcome> = self
      .outcome
      .as_deref()
      .map(|text| decode_json("invocation outcome", OUTCOME_SCHEMA, text))
      .transpose()?;
    Ok(Invocation {
      id: parse("invocation id", &self.id)?,
      run: parse("run id", &self.run)?,
      execution: parse("step execution id", &self.execution)?,
      number: u32::try_from(self.number).map_err(|_| corrupt("invocation number", self.number))?,
      kind: match self.kind.as_str() {
        AGENT => InvocationKind::Agent {
          model: ModelId::new(&self.model).map_err(|error| corrupt("invocation model", error))?,
        },
        COMMAND => InvocationKind::Command,
        other => return Err(corrupt("invocation kind", other)),
      },
      permissions: self.permissions.as_deref().map(launch_permissions).transpose()?,
      session: self.session,
      started_at: Timestamp::from_unix_millis(self.started_at),
      ended_at: self.ended_at.map(Timestamp::from_unix_millis),
      usage: TokenUsage {
        input: count(self.tokens[0])?,
        cached_input: count(self.tokens[1])?,
        output: count(self.tokens[2])?,
      },
      outcome,
      timing: InvocationTiming::restored(
        count(self.inactive_ms)?,
        self.waiting_since.map(Timestamp::from_unix_millis),
      ),
    })
  }
}

/// Stored launch permissions; those recorded with schema 1, before what happened to commands
/// outside them was, read with that unknown.
fn launch_permissions(text: &str) -> Result<LaunchPermissions, StoreError> {
  const WHAT: &str = "invocation permissions";
  match decode_document(WHAT, text)? {
    (PERMISSIONS_SCHEMA, body) => decode_body(WHAT, body),
    (1, body) => decode_body::<AgentPermissions>(WHAT, body).map(LaunchPermissions::recorded_before),
    (schema, _) => Err(corrupt(
      WHAT,
      format!("schema {schema} is not the supported schema {PERMISSIONS_SCHEMA}"),
    )),
  }
}
