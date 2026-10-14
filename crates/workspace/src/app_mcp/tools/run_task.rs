//! `run_task`: runs a task as the app lets a person run it. A draft is queued with its pipeline's
//! latest published version and started (Create & run); a queued task without a run starts its first
//! run (Start); a task whose latest run ended runs again when core's rule allows it (Run again,
//! keeping what the latest run left). A shared task is claimed first, as on the screens.

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  orchestrator::proposal::{ProposedStart, StartBasis, brief_excerpt, brief_is_cut},
  resource::{Pipeline, VersionNumber},
  run::{ListedRun, PreflightProblem, RunAgainBlock},
  store::{ResourceStore, RunStore},
  task::{Task, TaskStatus},
};
use orchester_store::{ProjectDb, RunStoreError};
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
  run_checked(services, args, None).await
}

/// Runs task `key` of `project` as [`run_task`] does, only as it was proposed: a task changed since, or a pipeline
/// published anew, is refused (`changed_since_proposed`) and starts nothing.
pub async fn run_proposed(
  services: &Services,
  (project, key): (String, String),
  basis: StartBasis,
) -> Result<Value, ToolError> {
  run_checked(services, RunTask { project, key }, Some(basis)).await
}

async fn run_checked(services: &Services, args: RunTask, expected: Option<StartBasis>) -> Result<Value, ToolError> {
  let prepared = services.clone();
  let (project, start) = blocking(move || {
    let project = prepared.project(&args.project)?;
    let start = prepare(&prepared, &project, &args.key, expected)?;
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

/// How the task named `key` runs; a refusal when the app would not let it run now, or, with `expected`, when it
/// would not run what was proposed.
fn prepare(
  services: &Services,
  project: &OpenProject,
  key: &str,
  expected: Option<StartBasis>,
) -> Result<Start, ToolError> {
  let db = ProjectDb::open(&project.repo_root)?;
  let task = find_task(&db, key)?;
  let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
  if let Some(expected) = expected
    && basis(services, (project, &db), &task, runs.last())? != expected
  {
    return Err(ToolError::refused(
      "changed_since_proposed",
      vec![Message::new("orchestrator.proposal.changed").with("task", task.key().to_string())],
    ));
  }
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

/// A start of a task the Orchestrator's agent proposed: its key, and what the person reads and approves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
  pub key: String,
  pub start: ProposedStart,
}

/// What task `task` of `project` would run now, its latest run `latest` read; a refusal when it could not run now,
/// as [`run_task`] would refuse it. A draft takes its pipeline's latest published version, any other task the one it
/// is pinned to.
fn basis(
  services: &Services,
  (project, db): (&OpenProject, &ProjectDb),
  task: &Task,
  latest: Option<&ListedRun<RunStoreError>>,
) -> Result<StartBasis, ToolError> {
  let version = match latest {
    None if task.status() == TaskStatus::Queued => task.pipeline_version(),
    None => Some(queue_version(db, task)?),
    Some(Err(_)) => return Err(unreadable()),
    Some(Ok(latest)) => {
      again(services, project, task, latest.id())?;
      task.pipeline_version()
    }
  };
  Ok(StartBasis {
    revision: task.revision().get(),
    pipeline: task.details().pipeline,
    version: version.map(VersionNumber::get),
  })
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
  let (task, basis, label) = blocking(move || {
    let project = services.project(&project)?;
    let db = ProjectDb::open(&project.repo_root)?;
    let task = find_task(&db, &key)?;
    let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
    let basis = basis(&services, (&project, &db), &task, runs.last())?;
    let published = published_pipelines(&db)?;
    let label = PipelineLabel::of(task.details().pipeline, &published).message();
    Ok((task, basis, label))
  })
  .await?;
  Ok(Proposal {
    key: task.key().to_string(),
    start: ProposedStart {
      pipeline: translate.text(&label),
      title: task.details().title.as_str().to_owned(),
      brief: brief_excerpt(&task.details().brief),
      brief_cut: brief_is_cut(&task.details().brief),
      criteria: task.details().acceptance_criteria.clone(),
      basis,
    },
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
