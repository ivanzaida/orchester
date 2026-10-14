//! `run_task`: runs a task as the app lets a person run it. A draft is queued with its pipeline's
//! latest published version and started (Create & run); a queued task without a run starts its first
//! run (Start); a task whose latest run ended runs again when core's rule allows it (Run again,
//! keeping what the latest run left). A shared task is claimed first, as on the screens.

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  resource::Pipeline,
  run::{PreflightProblem, RunAgainBlock},
  store::{ResourceStore, RunStore},
  task::{Task, TaskStatus},
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Translate, task_write::write_error, tasks::find_task};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  sharing::project::WorkClaim,
  tasks::{
    data::{SaveTarget, published_pipelines},
    detail::run_store_error,
    list::{PipelineLabel, Readiness},
    outside::Outside,
    run::{QueueFailure, preflight_message, queue_in, start_messages},
    run_again::{RunAgainCheck, block_message, request_messages},
  },
  workspace::OpenProject,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct RunTask {
  /// The project's id, or its name.
  pub project: String,
  /// The task's key, e.g. `KON-12`, or its id.
  pub key: String,
}

/// How the task runs, decided from what is stored.
enum Start {
  /// Its first run, of the queued task; `claim` was taken while queuing it.
  First { task: TaskId, claim: WorkClaim },
  /// Its next run after `after`, of the pipeline named `pipeline`.
  Again {
    task: TaskId,
    after: RunId,
    pipeline: String,
  },
}

pub async fn run_task(services: &Services, args: RunTask) -> Result<Value, ToolError> {
  let prepared = services.clone();
  let (project, start) = blocking(move || {
    let project = prepared.project(&args.project)?;
    let start = prepare(&prepared, &project, &args.key)?;
    Ok((project, start))
  })
  .await?;
  let (servers, runners) = (&services.servers, &services.runners);
  let root = project.repo_root.as_path();
  let (task, claim) = match &start {
    Start::First { task, .. } | Start::Again { task, .. } => (*task, servers.claim_for_work(root, *task).await),
  };
  let claim = claim.map_err(|refusal| ToolError::refused("claim_refused", vec![refusal.message()]))?;
  let (claim, started) = match start {
    Start::First { claim: queued, .. } => (
      queued.and(claim),
      runners
        .start_run_in(&project, task.first_run_key(), task)
        .await
        .map_err(|error| ToolError::runner(&error, start_messages(&error))),
    ),
    Start::Again { after, pipeline, .. } => (
      claim,
      runners
        .run_again_in(&project, IdempotencyKey::generate(), (task, after))
        .await
        .map_err(|error| ToolError::runner(&error, request_messages(&error, &pipeline))),
    ),
  };
  if started.is_err() {
    claim.release(servers).await;
  }
  Ok(json!({ "run": { "id": started?.to_string() } }))
}

/// How the task named `key` runs; a refusal when the app would not let it run now.
fn prepare(services: &Services, project: &OpenProject, key: &str) -> Result<Start, ToolError> {
  let db = ProjectDb::open(&project.repo_root)?;
  let task = find_task(&db, key)?;
  let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
  match runs.last() {
    None if task.status() == TaskStatus::Queued => Ok(Start::First {
      task: task.id(),
      claim: WorkClaim::default(),
    }),
    None => {
      let version = queue_version(&db, &task)?;
      drop(db);
      let target = SaveTarget::Task(task.id(), task.revision());
      let queued = queue_in(
        &project.repo_root,
        &services.servers,
        target,
        task.details().clone(),
        version,
      )
      .map_err(queue_failure)?;
      services.changes.record(project.project.id(), Outside::Tasks);
      Ok(Start::First {
        task: task.id(),
        claim: queued.claim,
      })
    }
    Some(Err(_)) => Err(unreadable()),
    Some(Ok(latest)) => again(services, project, &task, latest.id()),
  }
}

/// How the task runs again after its latest run `after`; a refusal when core's rule does not allow it now.
fn again(services: &Services, project: &OpenProject, task: &Task, after: RunId) -> Result<Start, ToolError> {
  let check = RunAgainCheck::load_in(&project.repo_root, &services.programs, task.id(), after)?;
  if let Err(block) = check.next {
    return Err(ToolError::refused(
      block_code(block),
      vec![block_message(block, &check.pipeline)],
    ));
  }
  Ok(Start::Again {
    task: task.id(),
    after,
    pipeline: check.pipeline.to_string(),
  })
}

/// A start of a task the Orchestrator's agent proposed: its key and the pipeline it would run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
  pub key: String,
  pub pipeline: String,
}

/// Checks that task `key` of `project` could run now as [`run_task`] would run it, without queuing or starting
/// anything: the Orchestrator's agent proposes the start and a person takes it. The pipeline is named in the words
/// of `translate`.
pub async fn propose_run(
  services: &Services,
  translate: &Translate,
  (project, key): (String, String),
) -> Result<Proposal, ToolError> {
  let services = services.clone();
  let (key, label) = blocking(move || {
    let project = services.project(&project)?;
    let db = ProjectDb::open(&project.repo_root)?;
    let task = find_task(&db, &key)?;
    let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
    match runs.last() {
      None if task.status() == TaskStatus::Queued => {}
      None => {
        queue_version(&db, &task)?;
      }
      Some(Err(_)) => return Err(unreadable()),
      Some(Ok(latest)) => {
        again(&services, &project, &task, latest.id())?;
      }
    }
    let published = published_pipelines(&db)?;
    Ok((
      task.key().to_string(),
      PipelineLabel::of(task.details().pipeline, &published).message(),
    ))
  })
  .await?;
  Ok(Proposal {
    key,
    pipeline: translate.text(&label),
  })
}

fn unreadable() -> ToolError {
  ToolError::refused("run_unreadable", vec![Message::new("runs.unreadable.rejected")])
}

/// The version a draft is queued with: its pipeline's latest published one, as the form offers it.
fn queue_version(db: &ProjectDb, task: &Task) -> Result<orchester_core::resource::VersionNumber, ToolError> {
  let readiness = Readiness::of(task);
  if readiness != Readiness::Ready {
    return Err(ToolError::refused("task_incomplete", vec![readiness.message()]));
  }
  let pipeline = task.details().pipeline.expect("a ready task has a pipeline");
  db.latest_version::<Pipeline>(pipeline)?
    .map(|version| version.number())
    .ok_or_else(|| {
      ToolError::refused(
        "pipeline_not_published",
        vec![preflight_message(&PreflightProblem::PipelineNotPublished)],
      )
    })
}

fn queue_failure(failure: QueueFailure) -> ToolError {
  let reasons = failure.messages();
  match failure {
    QueueFailure::Store(error) => error.into(),
    QueueFailure::Write(error) => write_error(*error),
    QueueFailure::Claim(_) => ToolError::refused("claim_refused", reasons),
    QueueFailure::Preflight(_) => ToolError::refused("preflight", reasons),
    QueueFailure::Queue(_) => ToolError::refused("task_incomplete", reasons),
    QueueFailure::Changed => ToolError::refused("changed_meanwhile", reasons),
  }
}

const fn block_code(block: RunAgainBlock) -> &'static str {
  match block {
    RunAgainBlock::TaskBlocked => "task_blocked",
    RunAgainBlock::NotEnded => "run_not_ended",
    RunAgainBlock::RunActive { .. } => "run_active",
    RunAgainBlock::PipelineNotPublished => "pipeline_not_published",
    RunAgainBlock::DailyBudgetUsed { .. } => "daily_budget_used",
  }
}
