//! What the Usage dashboard reads of a project: the usage of every completed turn and of every review
//! of auto mode's reviewer from a time on, with the invocations and runs they belong to, in a few
//! queries. A run whose record is damaged (its snapshot is not JSON) is left out with its invocations, so
//! the rest still reads; [`ProjectDb::unreadable_runs_since`] names the runs left out.

use std::{collections::HashMap, sync::Arc};

use orchester_core::{
  code_index::CodeIndexUse,
  connection::stream::AgentEvent,
  id::RunId,
  run::InvocationTiming,
  time::Timestamp,
  usage::{IndexGiven, InvocationFacts, RunFacts, RunOutcome, TurnUsage, UsageFacts},
};
use rusqlite::{OptionalExtension, Row};

use super::{ProjectDb, activity::EVENT_SCHEMA, code_index, connections::parse_provider, plan_limits::run_number};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, parse},
};

/// Invocations that ran at or after `?1`: started then, ended then, or still running; or whose request a
/// review judged then.
const RAN_SINCE: &str = "invocations.started_at >= ?1 OR invocations.ended_at >= ?1 OR invocations.ended_at IS NULL \
                         OR invocations.id IN (SELECT invocation_id FROM reviewer_usage WHERE ended_at >= ?1)";

/// The runs whose record reads: SQLite's JSON functions fail the whole query on one snapshot that is not JSON.
const READABLE_RUNS: &str = "(SELECT * FROM runs WHERE json_valid(snapshot))";

impl ProjectDb {
  /// The usage of the turns that completed from `since` on, the invocations that ran from then on
  /// and their runs; running invocations are measured to `now`.
  pub fn usage_facts(&self, since: Timestamp, now: Timestamp) -> Result<UsageFacts, StoreError> {
    Ok(UsageFacts {
      turns: self.turns_since(since)?,
      reviews: self.reviews_since(since)?,
      invocations: self.invocations_since(since, now)?,
      runs: self.runs_since(since)?,
      last_turn: self
        .connection
        .query_row(
          "SELECT MAX(at) FROM agent_events WHERE kind = 'turn_completed'",
          [],
          |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten()
        .map(Timestamp::from_unix_millis),
    })
  }

  /// The runs with invocations from `since` on whose record is damaged, which [`Self::usage_facts`] leaves out.
  pub fn unreadable_runs_since(&self, since: Timestamp) -> Result<Vec<RunId>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT id FROM runs WHERE NOT json_valid(snapshot) \
       AND id IN (SELECT run_id FROM invocations WHERE {RAN_SINCE}) ORDER BY number"
    ))?;
    let ids = statement
      .query_map([since.unix_millis()], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    ids.iter().map(|id| parse("run id", id)).collect()
  }

  fn turns_since(&self, since: Timestamp) -> Result<Vec<TurnUsage>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT at, invocation_id, payload FROM agent_events WHERE kind = 'turn_completed' AND at >= ?1 ORDER BY at",
    )?;
    let rows = statement
      .query_map([since.unix_millis()], |row| {
        Ok((
          row.get::<_, i64>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(
        |(at, invocation, payload)| match decode_json("agent event", EVENT_SCHEMA, &payload)? {
          AgentEvent::TurnCompleted { usage } => Ok(TurnUsage {
            at: Timestamp::from_unix_millis(at),
            invocation: parse("invocation id", &invocation)?,
            usage,
          }),
          other => Err(corrupt(
            "agent event",
            format!("a `turn_completed` row holds `{}`", other.name()),
          )),
        },
      )
      .collect()
  }

  fn invocations_since(&self, since: Timestamp, now: Timestamp) -> Result<Vec<InvocationFacts>, StoreError> {
    // The step's name as the run's pinned pipeline has it, or its key when the snapshot lacks it.
    let mut statement = self.connection.prepare(&format!(
      "SELECT invocations.id, invocations.run_id, invocations.kind, invocations.model, \
         COALESCE((SELECT json_extract(step.value, '$.name') FROM json_each(runs.snapshot, '$.body.body.steps') AS step \
           WHERE json_extract(step.value, '$.key') = executions.step_key), executions.step_key), \
         profiles.name, connections.provider, invocations.started_at, invocations.ended_at, invocations.inactive_ms, \
         invocations.waiting_since, index_use.body, edits.body, profiles.id, accounts.account_id \
       FROM invocations \
       JOIN {READABLE_RUNS} AS runs ON runs.id = invocations.run_id \
       JOIN step_executions AS executions ON executions.id = invocations.step_execution_id \
       LEFT JOIN resource_versions AS versions ON versions.id = executions.profile_version_id \
       LEFT JOIN resources AS profiles ON profiles.id = versions.resource_id \
       LEFT JOIN connections ON connections.id = json_extract(versions.body, '$.body.connection') \
       LEFT JOIN invocation_code_index AS index_use ON index_use.invocation_id = invocations.id \
       LEFT JOIN invocation_edit_progress AS edits ON edits.invocation_id = invocations.id \
       LEFT JOIN invocation_accounts AS accounts ON accounts.invocation_id = invocations.id \
       WHERE {RAN_SINCE} ORDER BY invocations.started_at, invocations.rowid"
    ))?;
    let rows = statement
      .query_map([since.unix_millis()], InvocationRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(|row| row.into_facts(now)).collect()
  }

  fn runs_since(&self, since: Timestamp) -> Result<Vec<RunFacts>, StoreError> {
    let tags = self.task_tag_names()?;
    let mut statement = self.connection.prepare(&format!(
      "SELECT runs.id, runs.number, runs.task_id, runs.status, tasks.number, tasks.title, pipelines.name, pipelines.id, \
       json_extract(runs.snapshot, '$.body.pipeline.number') \
       FROM {READABLE_RUNS} AS runs JOIN tasks ON tasks.id = runs.task_id \
       LEFT JOIN resources AS pipelines ON pipelines.id = json_extract(runs.snapshot, '$.body.pipeline.resource') \
       WHERE runs.id IN (SELECT run_id FROM invocations WHERE {RAN_SINCE}) ORDER BY runs.number"
    ))?;
    let rows = statement
      .query_map([since.unix_millis()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, i64>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, String>(3)?,
          row.get::<_, i64>(4)?,
          row.get::<_, String>(5)?,
          row.get::<_, Option<String>>(6)?,
          row.get::<_, Option<String>>(7)?,
          row.get::<_, Option<i64>>(8)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    let key = self.project().key().as_str().to_owned();
    rows
      .into_iter()
      .map(
        |(id, number, task, status, task_number, title, pipeline, pipeline_id, version)| {
          Ok(RunFacts {
            id: parse("run id", &id)?,
            number: run_number(number)?,
            tags: tags.get(&task).cloned().unwrap_or_default(),
            task: parse("task id", &task)?,
            task_key: format!("{key}-{task_number}").into(),
            task_title: title.into(),
            pipeline: pipeline.map(Arc::from),
            pipeline_id: pipeline_id.as_deref().map(|id| parse("pipeline id", id)).transpose()?,
            pipeline_version: version.map(super::resources::version_number).transpose()?,
            outcome: outcome(&status)?,
          })
        },
      )
      .collect()
  }

  /// The names of each task's tags, by task id.
  fn task_tag_names(&self) -> Result<HashMap<String, Vec<Arc<str>>>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT organization.task_id, tags.name \
       FROM task_organization AS organization, json_each(organization.body, '$.body.tags') AS tag \
       JOIN project_tags AS tags ON tags.id = tag.value ORDER BY tags.normalized",
    )?;
    let rows = statement
      .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    let mut names: HashMap<String, Vec<Arc<str>>> = HashMap::new();
    for (task, name) in rows {
      names.entry(task).or_default().push(name.into());
    }
    Ok(names)
  }
}

fn outcome(status: &str) -> Result<RunOutcome, StoreError> {
  Ok(match status {
    "pending" => RunOutcome::Pending,
    "running" => RunOutcome::Running,
    "paused" => RunOutcome::Paused,
    "waiting" => RunOutcome::Waiting,
    "interrupted" => RunOutcome::Interrupted,
    "succeeded" => RunOutcome::Succeeded,
    "failed" => RunOutcome::Failed,
    "cancelled" => RunOutcome::Cancelled,
    other => return Err(corrupt("run status", other)),
  })
}

struct InvocationRow {
  id: String,
  run: String,
  kind: String,
  model: String,
  step: String,
  profile: Option<String>,
  provider: Option<String>,
  started_at: i64,
  ended_at: Option<i64>,
  inactive_ms: i64,
  waiting_since: Option<i64>,
  index: Option<String>,
  edit: Option<String>,
  profile_id: Option<String>,
  account: Option<String>,
}

impl InvocationRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      run: row.get(1)?,
      kind: row.get(2)?,
      model: row.get(3)?,
      step: row.get(4)?,
      profile: row.get(5)?,
      provider: row.get(6)?,
      started_at: row.get(7)?,
      ended_at: row.get(8)?,
      inactive_ms: row.get(9)?,
      waiting_since: row.get(10)?,
      index: row.get(11)?,
      edit: row.get(12)?,
      profile_id: row.get(13)?,
      account: row.get(14)?,
    })
  }

  fn into_facts(self, now: Timestamp) -> Result<InvocationFacts, StoreError> {
    let agent = match self.kind.as_str() {
      "agent" => true,
      "command" => false,
      other => return Err(corrupt("invocation kind", other)),
    };
    let inactive = u64::try_from(self.inactive_ms).map_err(|_| corrupt("inactive time", self.inactive_ms))?;
    let timing = InvocationTiming::restored(inactive, self.waiting_since.map(Timestamp::from_unix_millis));
    let (started_at, ended_at) = (
      Timestamp::from_unix_millis(self.started_at),
      self.ended_at.map(Timestamp::from_unix_millis),
    );
    let index = match self.index.as_deref().map(code_index::decode_use).transpose()? {
      Some(CodeIndexUse::Given { .. }) => IndexGiven::Yes,
      Some(CodeIndexUse::Off | CodeIndexUse::Unavailable { .. }) => IndexGiven::No,
      None => IndexGiven::Unknown,
    };
    Ok(InvocationFacts {
      id: parse("invocation id", &self.id)?,
      run: parse("run id", &self.run)?,
      cli: if agent {
        self.provider.as_deref().map(parse_provider).transpose()?
      } else {
        None
      },
      model: agent.then(|| Arc::from(self.model.as_str())),
      profile: self.profile.filter(|_| agent).map(Arc::from),
      profile_id: self
        .profile_id
        .filter(|_| agent)
        .map(|id| parse("profile id", &id))
        .transpose()?,
      account: self.account.map(|account| parse("account id", &account)).transpose()?,
      step: self.step.into(),
      started_at,
      ended_at,
      active_ms: u64::try_from(timing.active_millis(started_at, ended_at, now)).unwrap_or(0),
      edit: self.edit.as_deref().map(code_index::decode_progress).transpose()?,
      index,
    })
  }
}
