//! The cache of a shared project's tasks (`docs/architecture/server.md`, "Desktop data"): what the
//! server confirmed, written by the desktop. A cached task is an ordinary row of `tasks`, so the
//! runner and every local screen read it as before; it also keeps its pipeline's slug, its former
//! key, who created and last changed it, its claim and the run status its holder reported.

use std::num::NonZeroU32;

use orchester_core::{
  id::{ClaimId, TaskId, UserId},
  name::Name,
  resource::VersionNumber,
  run::TaskPhase,
  sharing::{ClaimKind, TaskRevision},
  task::{Task, TaskDetails, TaskLimits, TaskStatus, organization::TaskOrganization},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{
  ProjectDb, organization,
  sharing::{self, SharingLink},
  tasks,
};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, parse, tokens_column},
};

/// A member of the server project, as the cache names them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedPerson {
  pub id: UserId,
  pub name: String,
}

/// Who holds a shared task, as last read from the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedClaim {
  pub id: ClaimId,
  pub holder: SharedPerson,
  pub kind: ClaimKind,
  pub since: Timestamp,
  pub heartbeat_at: Timestamp,
  pub stale: bool,
  pub taken_over_from: Option<SharedPerson>,
}

/// The short status of a shared task's latest run that its holder reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedRunStatus {
  pub holder: SharedPerson,
  pub number: NonZeroU32,
  pub phase: TaskPhase,
  pub step: Option<String>,
  pub started_at: Timestamp,
  pub phase_since: Timestamp,
  pub updated_at: Timestamp,
}

/// A shared task as the server confirmed it. Its pipeline is a slug: pipelines are not shared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedTask {
  pub id: TaskId,
  pub number: NonZeroU32,
  pub revision: TaskRevision,
  pub title: Name,
  pub brief: String,
  pub criteria: Vec<String>,
  pub pipeline: Option<String>,
  pub limits: TaskLimits,
  pub status: TaskStatus,
  pub organization: TaskOrganization,
  pub former_key: Option<String>,
  pub created_by: String,
  pub created_at: Timestamp,
  pub updated_by: String,
  pub updated_at: Timestamp,
  pub claim: Option<CachedClaim>,
  pub run: Option<CachedRunStatus>,
}

/// What the cache knows of a shared task beyond the task itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedTaskInfo {
  /// The slug of the task's pipeline; the task's pipeline is none when this computer has none
  /// of that slug.
  pub pipeline_slug: Option<String>,
  pub former_key: Option<String>,
  pub created_by: Option<String>,
  pub updated_by: Option<String>,
  pub updated_at: Option<Timestamp>,
  pub claim: Option<CachedClaim>,
  pub run: Option<CachedRunStatus>,
  /// While the task is *Not shared*: the names of its tags, kept until it is shared.
  pub kept_tags: Option<Vec<String>>,
}

impl ProjectDb {
  /// Shares the project (*Share*, the first person): records `link`, makes the server project's
  /// `tags` the project's tags and writes the uploaded `tasks` as the server confirmed them, in one
  /// transaction.
  /// Links the project to its server project and caches what the server confirmed. Returns the
  /// tasks as they were here, read in the same transaction, whose revision is no longer the one
  /// `uploaded` read: a change made while sharing (a run's), which the caller sends on top.
  pub fn start_sharing(
    &mut self,
    link: &SharingLink,
    tags: &[orchester_core::task::organization::Tag],
    (tasks, uploaded): (&[CachedTask], &std::collections::HashMap<TaskId, TaskRevision>),
  ) -> Result<Vec<Task>, StoreError> {
    // Immediate: the runner may write between the read and the first write of a deferred one.
    let transaction = self
      .connection
      .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if sharing::link(&transaction)?.is_some() {
      return Err(StoreError::AlreadyShared);
    }
    let project = project_key(&transaction)?;
    let changed = tasks::load_tasks(&transaction, &project, "", [])?
      .into_iter()
      .filter(|task| uploaded.get(&task.id()).is_some_and(|read| *read != task.revision()))
      .collect();
    sharing::insert_link(&transaction, link)?;
    replace_tags(&transaction, tags)?;
    for task in tasks {
      write_cached(&transaction, task, None)?;
    }
    transaction.commit()?;
    Ok(changed)
  }

  /// Writes `tasks` as the server confirmed them; `batch` is the key a creation of them was sent
  /// with, so a retry finds them. Earlier runs of a task whose content changed keep the content
  /// they started with.
  pub fn cache_shared_tasks(&mut self, tasks: &[CachedTask], batch: Option<&str>) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    if sharing::link(&transaction)?.is_none() {
      return Err(StoreError::NotShared);
    }
    for task in tasks {
      write_cached(&transaction, task, batch)?;
    }
    transaction.commit()?;
    Ok(())
  }

  /// Makes the server project's `tags` the project's tags, as the server lists them now.
  pub fn cache_shared_tags(&mut self, tags: &[orchester_core::task::organization::Tag]) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    if sharing::link(&transaction)?.is_none() {
      return Err(StoreError::NotShared);
    }
    replace_tags(&transaction, tags)?;
    transaction.commit()?;
    Ok(())
  }

  /// Replaces the cached claim of `task`, e.g. one a heartbeat reported lost.
  pub fn cache_claim(&mut self, task: TaskId, claim: Option<&CachedClaim>) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    write_claim(&transaction, task, claim)?;
    transaction.commit()?;
    Ok(())
  }

  /// Pins the pipeline version a queued shared task runs on this computer (`None` unpins it): the
  /// server never has it, so it changes no revision.
  pub fn pin_shared_task(&mut self, task: TaskId, version: Option<VersionNumber>) -> Result<(), StoreError> {
    let updated = self.connection.execute(
      "UPDATE tasks SET pipeline_version = ?2 WHERE id = ?1 AND sharing = 'shared'",
      params![task.to_string(), version.map(VersionNumber::get)],
    )?;
    if updated == 0 {
      return Err(StoreError::TaskNotFound(task));
    }
    Ok(())
  }

  pub fn shared_task_info(&self, task: TaskId) -> Result<Option<SharedTaskInfo>, StoreError> {
    info(&self.connection, task)
  }

  /// The shared tasks whose pipeline this computer has no pipeline for, with the pipeline's slug:
  /// they show it as missing, and an edit sends no pipeline change for them.
  pub fn missing_pipelines(&self) -> Result<std::collections::HashMap<TaskId, String>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT id, pipeline_slug FROM tasks WHERE sharing IS NOT NULL AND pipeline_slug IS NOT NULL        AND NOT EXISTS (SELECT 1 FROM resources WHERE resources.kind = 'pipeline'        AND resources.slug = tasks.pipeline_slug)",
    )?;
    let rows = statement
      .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(id, slug)| Ok((parse("task id", &id)?, slug)))
      .collect()
  }

  /// The cached tasks whose claim `holder` holds, which this computer's heartbeat keeps fresh.
  pub fn claims_held_by(&self, holder: UserId) -> Result<Vec<TaskId>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT task_id FROM task_claims WHERE holder_id = ?1 ORDER BY task_id")?;
    let ids = statement
      .query_map([holder.to_string()], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter().map(|id| parse("task id", &id)).collect()
  }
}

/// Whether `task` is a shared task.
pub(super) fn is_shared(connection: &Connection, task: TaskId) -> Result<bool, StoreError> {
  Ok(connection.query_row(
    "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1 AND sharing = 'shared')",
    [task.to_string()],
    |row| row.get(0),
  )?)
}

/// Whether `task` was kept *Not shared* when the project joined.
pub(super) fn is_not_shared(connection: &Connection, task: TaskId) -> Result<bool, StoreError> {
  Ok(connection.query_row(
    "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1 AND sharing = 'not_shared')",
    [task.to_string()],
    |row| row.get(0),
  )?)
}

pub(super) fn replace_tags(
  connection: &Connection,
  tags: &[orchester_core::task::organization::Tag],
) -> Result<(), StoreError> {
  connection.execute("DELETE FROM project_tags", [])?;
  for tag in tags {
    connection.execute(
      "INSERT INTO project_tags (id, name, normalized) VALUES (?1, ?2, ?3)",
      params![
        tag.id().to_string(),
        tag.name().as_str(),
        tag.name().as_str().to_lowercase()
      ],
    )?;
  }
  Ok(())
}

/// Writes `cached` over the stored row of its id, or as a new row. The local pin of the pipeline
/// version stays while the task stays queued: a version number means nothing on another computer.
pub(super) fn write_cached(
  transaction: &Transaction<'_>,
  cached: &CachedTask,
  batch: Option<&str>,
) -> Result<(), StoreError> {
  let project = project_key(transaction)?;
  let before = tasks::load_tasks(transaction, &project, "WHERE id = ?1", [cached.id.to_string()])?.pop();
  if let Some(before) = before.as_ref().filter(|before| content_differs(before, cached)) {
    tasks::snapshot_earlier_runs(transaction, before)?;
  }
  let (budget, duration) = tasks::limit_columns(cached.limits)?;
  transaction.execute(
    "INSERT INTO tasks (id, number, batch_key, title, brief, pipeline_id, budget_tokens, max_duration_ms, status, \
     created_at, pipeline_version, revision, sharing, pipeline_slug, former_key, created_by, updated_by, updated_at) \
     VALUES (?1, ?2, COALESCE(?3, ?1), ?4, ?5, NULL, ?6, ?7, ?8, ?9, NULL, ?10, 'shared', ?11, ?12, ?13, ?14, ?15) \
     ON CONFLICT (id) DO UPDATE SET number = excluded.number, title = excluded.title, brief = excluded.brief, \
     pipeline_id = NULL, budget_tokens = excluded.budget_tokens, max_duration_ms = excluded.max_duration_ms, \
     status = excluded.status, created_at = excluded.created_at, \
     pipeline_version = CASE WHEN excluded.status = 'queued' THEN tasks.pipeline_version END, \
     revision = excluded.revision, sharing = 'shared', pipeline_slug = excluded.pipeline_slug, \
     former_key = excluded.former_key, created_by = excluded.created_by, updated_by = excluded.updated_by, \
     updated_at = excluded.updated_at, kept_key = NULL, kept_tag_names = NULL",
    params![
      cached.id.to_string(),
      cached.number.get(),
      batch,
      cached.title.as_str(),
      cached.brief,
      budget,
      duration,
      tasks::status_text(cached.status),
      cached.created_at.unix_millis(),
      tokens_column("task revision", cached.revision.get())?,
      cached.pipeline,
      cached.former_key,
      cached.created_by,
      cached.updated_by,
      cached.updated_at.unix_millis(),
    ],
  )?;
  tasks::write_criteria(transaction, cached.id, &cached.criteria)?;
  organization::store_cached(transaction, cached.id, &cached.organization, cached.updated_at)?;
  write_claim(transaction, cached.id, cached.claim.as_ref())?;
  write_run_status(transaction, cached.id, cached.run.as_ref())
}

fn content_differs(before: &Task, cached: &CachedTask) -> bool {
  let details: &TaskDetails = before.details();
  details.title != cached.title
    || details.brief != cached.brief
    || details.acceptance_criteria != cached.criteria
    || details.limits != cached.limits
}

pub(super) fn project_key(connection: &Connection) -> Result<orchester_core::project::ProjectKey, StoreError> {
  let key: String = connection.query_row("SELECT key FROM project", [], |row| row.get(0))?;
  orchester_core::project::ProjectKey::new(&key).map_err(|error| corrupt("project key", error))
}

fn write_claim(connection: &Connection, task: TaskId, claim: Option<&CachedClaim>) -> Result<(), StoreError> {
  connection.execute("DELETE FROM task_claims WHERE task_id = ?1", [task.to_string()])?;
  let Some(claim) = claim else {
    return Ok(());
  };
  connection.execute(
    "INSERT INTO task_claims (task_id, claim_id, holder_id, holder_name, kind, since, heartbeat_at, stale, \
     taken_over_from_id, taken_over_from_name) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    params![
      task.to_string(),
      claim.id.to_string(),
      claim.holder.id.to_string(),
      claim.holder.name,
      claim_kind_text(claim.kind),
      claim.since.unix_millis(),
      claim.heartbeat_at.unix_millis(),
      claim.stale,
      claim.taken_over_from.as_ref().map(|person| person.id.to_string()),
      claim.taken_over_from.as_ref().map(|person| person.name.clone()),
    ],
  )?;
  Ok(())
}

fn write_run_status(connection: &Connection, task: TaskId, run: Option<&CachedRunStatus>) -> Result<(), StoreError> {
  connection.execute("DELETE FROM task_run_status WHERE task_id = ?1", [task.to_string()])?;
  let Some(run) = run else {
    return Ok(());
  };
  connection.execute(
    "INSERT INTO task_run_status (task_id, holder_id, holder_name, number, phase, step, started_at, phase_since, \
     updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    params![
      task.to_string(),
      run.holder.id.to_string(),
      run.holder.name,
      run.number.get(),
      phase_text(run.phase),
      run.step,
      run.started_at.unix_millis(),
      run.phase_since.unix_millis(),
      run.updated_at.unix_millis(),
    ],
  )?;
  Ok(())
}

type InfoRow = (
  Option<String>,
  Option<String>,
  Option<String>,
  Option<String>,
  Option<i64>,
  Option<String>,
);

fn info(connection: &Connection, task: TaskId) -> Result<Option<SharedTaskInfo>, StoreError> {
  let row: Option<InfoRow> = connection
    .query_row(
      "SELECT pipeline_slug, former_key, created_by, updated_by, updated_at, kept_tag_names FROM tasks \
       WHERE id = ?1 AND sharing IS NOT NULL",
      [task.to_string()],
      |row| {
        Ok((
          row.get(0)?,
          row.get(1)?,
          row.get(2)?,
          row.get(3)?,
          row.get(4)?,
          row.get(5)?,
        ))
      },
    )
    .optional()?;
  let Some((pipeline_slug, former_key, created_by, updated_by, updated_at, kept)) = row else {
    return Ok(None);
  };
  let kept_tags = kept.map(|names| decode_json("kept tag names", 1, &names)).transpose()?;
  Ok(Some(SharedTaskInfo {
    pipeline_slug,
    former_key,
    created_by,
    updated_by,
    updated_at: updated_at.map(Timestamp::from_unix_millis),
    claim: read_claim(connection, task)?,
    run: read_run_status(connection, task)?,
    kept_tags,
  }))
}

type ClaimRow = (
  String,
  String,
  String,
  String,
  i64,
  i64,
  bool,
  Option<String>,
  Option<String>,
);

fn read_claim(connection: &Connection, task: TaskId) -> Result<Option<CachedClaim>, StoreError> {
  let row: Option<ClaimRow> = connection
    .query_row(
      "SELECT claim_id, holder_id, holder_name, kind, since, heartbeat_at, stale, taken_over_from_id, \
       taken_over_from_name FROM task_claims WHERE task_id = ?1",
      [task.to_string()],
      |row| {
        Ok((
          row.get(0)?,
          row.get(1)?,
          row.get(2)?,
          row.get(3)?,
          row.get(4)?,
          row.get(5)?,
          row.get(6)?,
          row.get(7)?,
          row.get(8)?,
        ))
      },
    )
    .optional()?;
  let Some((id, holder, holder_name, kind, since, heartbeat_at, stale, from_id, from_name)) = row else {
    return Ok(None);
  };
  let taken_over_from = match (from_id, from_name) {
    (Some(id), Some(name)) => Some(SharedPerson {
      id: parse("claim holder", &id)?,
      name,
    }),
    _ => None,
  };
  Ok(Some(CachedClaim {
    id: parse("claim id", &id)?,
    holder: SharedPerson {
      id: parse("claim holder", &holder)?,
      name: holder_name,
    },
    kind: parse_claim_kind(&kind)?,
    since: Timestamp::from_unix_millis(since),
    heartbeat_at: Timestamp::from_unix_millis(heartbeat_at),
    stale,
    taken_over_from,
  }))
}

type RunStatusRow = (String, String, i64, String, Option<String>, i64, i64, i64);

fn read_run_status(connection: &Connection, task: TaskId) -> Result<Option<CachedRunStatus>, StoreError> {
  let row: Option<RunStatusRow> = connection
    .query_row(
      "SELECT holder_id, holder_name, number, phase, step, started_at, phase_since, updated_at \
       FROM task_run_status WHERE task_id = ?1",
      [task.to_string()],
      |row| {
        Ok((
          row.get(0)?,
          row.get(1)?,
          row.get(2)?,
          row.get(3)?,
          row.get(4)?,
          row.get(5)?,
          row.get(6)?,
          row.get(7)?,
        ))
      },
    )
    .optional()?;
  let Some((holder, name, number, phase, step, started_at, phase_since, updated_at)) = row else {
    return Ok(None);
  };
  Ok(Some(CachedRunStatus {
    holder: SharedPerson {
      id: parse("run holder", &holder)?,
      name,
    },
    number: u32::try_from(number)
      .ok()
      .and_then(NonZeroU32::new)
      .ok_or_else(|| corrupt("run status number", number))?,
    phase: parse_phase(&phase)?,
    step,
    started_at: Timestamp::from_unix_millis(started_at),
    phase_since: Timestamp::from_unix_millis(phase_since),
    updated_at: Timestamp::from_unix_millis(updated_at),
  }))
}

const fn claim_kind_text(kind: ClaimKind) -> &'static str {
  match kind {
    ClaimKind::Manual => "manual",
    ClaimKind::Run => "run",
  }
}

fn parse_claim_kind(value: &str) -> Result<ClaimKind, StoreError> {
  match value {
    "manual" => Ok(ClaimKind::Manual),
    "run" => Ok(ClaimKind::Run),
    other => Err(corrupt("claim kind", other)),
  }
}

const fn phase_text(phase: TaskPhase) -> &'static str {
  match phase {
    TaskPhase::Draft => "draft",
    TaskPhase::Queued => "queued",
    TaskPhase::Running => "running",
    TaskPhase::Waiting => "waiting",
    TaskPhase::Stuck => "stuck",
    TaskPhase::Paused => "paused",
    TaskPhase::StoppedAtLimit => "stopped_at_limit",
    TaskPhase::Interrupted => "interrupted",
    TaskPhase::Succeeded => "succeeded",
    TaskPhase::Merged => "merged",
    TaskPhase::Failed => "failed",
    TaskPhase::Cancelled => "cancelled",
  }
}

fn parse_phase(value: &str) -> Result<TaskPhase, StoreError> {
  Ok(match value {
    "draft" => TaskPhase::Draft,
    "queued" => TaskPhase::Queued,
    "running" => TaskPhase::Running,
    "waiting" => TaskPhase::Waiting,
    "stuck" => TaskPhase::Stuck,
    "paused" => TaskPhase::Paused,
    "stopped_at_limit" => TaskPhase::StoppedAtLimit,
    "interrupted" => TaskPhase::Interrupted,
    "succeeded" => TaskPhase::Succeeded,
    "merged" => TaskPhase::Merged,
    "failed" => TaskPhase::Failed,
    "cancelled" => TaskPhase::Cancelled,
    other => return Err(corrupt("run status phase", other)),
  })
}
