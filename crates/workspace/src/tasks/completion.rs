//! A run that passed every step and committed nothing (ORC-93), as its task's Summary, Needs you and automation read
//! it: what the run delivers, whether it was completed and by whom, its report, and for work done outside the
//! repository the MCP tool calls that changed something. A run whose result is its report is completed (its task Done)
//! instead of merged; one whose pipeline expects changes is flagged and waits for the person, who may complete it
//! anyway. Closing the task while such a run waits settles its completion (ORC-150, [`settle_closed_in`]).

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::RunId,
  integration::RunWork,
  orchester_tools,
  resource::{Deliverable, StepKey, WithoutCommits},
  run::{
    CompletedBy, DecidedBy, DecidedToolApproval, Plan, REPORT_ARTIFACT, Run, RunArtifact, RunCompletion, RunStatus,
    StepOutput, ToolApprovalDecision, last_report, run_delivers,
  },
  store::{ActivityStore, RunStore, TaskStore},
  task::{Task, organization::ChangeActor},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::{
  delivery::{self, Delivery},
  detail::run_store_error,
};
use crate::messages::{self, Message};

/// What became of a run that passed every step, beyond a merge: what it delivers, its completion, its report and,
/// for work done outside the repository, the tool calls that changed something.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunResult {
  pub delivers: Deliverable,
  pub completion: Option<RunCompletion>,
  /// The run's report: its kept *Report* once it was completed, else what its last step reported; with its step.
  pub report: Option<Report>,
  /// For a run that delivers work outside the repository, the MCP tool calls it was let make, oldest first.
  pub outside: Vec<OutsideChange>,
}

/// A run's report and the step that wrote it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
  pub step: Option<StepKey>,
  pub text: Arc<str>,
}

/// An MCP tool call a run was let make outside Orchester's own tools: the server, the tool, what it acted on, when it
/// was approved and who approved it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutsideChange {
  pub server: Arc<str>,
  pub tool: Arc<str>,
  pub target: Option<Arc<str>>,
  pub at: Timestamp,
  /// `None` for a request answered before answers were recorded with who gave them.
  pub by: Option<DecidedBy>,
}

impl RunResult {
  /// What `run` of `task`, with its step `outputs` and `plans`, left beyond a merge, as `db` keeps it.
  pub fn read(
    db: &ProjectDb,
    task: &Task,
    run: &Run,
    (outputs, plans): (&[StepOutput], &[Plan]),
  ) -> Result<Self, StoreError> {
    let delivers = run_delivers(task, run);
    let kept = db
      .run_artifacts(run.id())?
      .into_iter()
      .find(|artifact| artifact.name == REPORT_ARTIFACT);
    let report = match kept {
      Some(kept) => Some(Report {
        step: kept.step,
        text: kept.text.into(),
      }),
      None => last_report(run, outputs, plans).map(|(step, text)| Report {
        step: Some(step.clone()),
        text: text.into(),
      }),
    };
    let outside = if delivers == Deliverable::Outside {
      outside_changes(&db.decided_tool_approvals(run.id())?)
    } else {
      Vec::new()
    };
    Ok(Self {
      delivers,
      completion: db.run_completion(run.id())?,
      report,
      outside,
    })
  }

  /// What the run waits for, now that it committed nothing: to be completed, or the person's decision.
  pub const fn without_commits(&self) -> WithoutCommits {
    self.delivers.without_commits()
  }
}

/// The MCP tool calls among `decided` that were approved, but those of Orchester's own tools.
pub fn outside_changes(decided: &[DecidedToolApproval]) -> Vec<OutsideChange> {
  decided
    .iter()
    .filter(|decided| {
      let request = &decided.request;
      !request.command
        && request.server != orchester_tools::SERVER
        && decided.decision == Some(ToolApprovalDecision::Approved)
    })
    .map(|decided| OutsideChange {
      server: decided.request.server.as_str().into(),
      tool: decided.request.action.as_str().into(),
      target: decided.request.target.as_deref().map(Arc::from),
      at: decided.decided_at,
      by: decided.decided_by,
    })
    .collect()
}

/// Completes run `run` of the project at `repo_root`, by `by`, at `now`: its task's latest run that passed every step
/// and committed nothing past the revision it started from, as git through `programs` tells. Its report is kept as its
/// artifact *Report* and its task is Done. A run whose pipeline expects changes is completed only when `anyway` (a
/// person's *Complete anyway*). Blocks on SQLite and git.
pub fn complete_in(
  (repo_root, programs): (&Path, &Programs),
  run: RunId,
  (by, anyway): (CompletedBy, bool),
  now: Timestamp,
) -> Result<RunCompletion, Message> {
  let store = |error: StoreError| messages::store_error(&error);
  let mut db = ProjectDb::open(repo_root).map_err(store)?;
  let found = db.run(run).map_err(run_store_error).map_err(store)?;
  let Some(found) = found else {
    return Err(Message::new("runs.complete.failure.notCompletable"));
  };
  let task = db
    .task(found.task())
    .map_err(store)?
    .ok_or_else(|| Message::new("runs.complete.failure.notCompletable"))?;
  if *found.status() != RunStatus::Succeeded {
    return Err(Message::new("runs.complete.failure.notCompletable"));
  }
  if delivery::of_run(programs, repo_root, &found) != Delivery::NothingToMerge {
    return Err(Message::new("runs.complete.failure.hasCommits"));
  }
  if run_delivers(&task, &found).without_commits() == WithoutCommits::Flagged && !anyway {
    return Err(Message::new("runs.complete.failure.expectsChanges"));
  }
  let report = report_artifact(&db, &found, now).map_err(store)?;
  let completion = RunCompletion {
    run,
    by,
    completed_at: now,
  };
  db.save_run_completion(&completion, report.as_ref()).map_err(store)?;
  Ok(completion)
}

/// Settles the completion of each closed task's latest run that waits to be completed (ORC-150), in the project at
/// `repo_root`: a task closed, Done or Canceled, while its run passed every step and committed nothing, as git through
/// `programs` tells, is completed as of its closing, by whoever closed it ([`closer`]), its report kept as completing
/// keeps it, and its status left as it was closed. However it was closed (on its page, on the board, through
/// Orchester's MCP server, or by a member on the team server), its Summary and every *Needs you* stop asking. Whether
/// it settled any. Blocks on SQLite, and on git for such runs only.
pub fn settle_closed_in((repo_root, programs): (&Path, &Programs)) -> Result<bool, StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  let mut waiting = Vec::new();
  for task in db.tasks()? {
    if !task.organization().status().is_closed() {
      continue;
    }
    if let Some(run) = db.uncompleted_latest_run(task.id())?
      && let Some(run) = db.run(run).map_err(run_store_error)?
    {
      waiting.push((task, run));
    }
  }
  let works: Vec<RunWork> = waiting.iter().map(|(_, run)| RunWork::of(run)).collect();
  let deliveries = delivery::deliveries(programs, repo_root, &works);
  let mut settled = false;
  for ((task, run), delivery) in waiting.iter().zip(deliveries) {
    if delivery != Delivery::NothingToMerge {
      continue;
    }
    let (by, at) = closer(&db, task)?;
    let report = report_artifact(&db, run, at)?;
    let completion = RunCompletion {
      run: run.id(),
      by,
      completed_at: at,
    };
    settled |= db.settle_run_completion(&completion, report.as_ref())?;
  }
  Ok(settled)
}

/// Who closed `task`, a closed task, and when: a shared task a member of its project, on its team server, when the
/// server last changed it; a local task the actor of the change that closed it, a person or automation through
/// Orchester's MCP server, and when.
fn closer(db: &ProjectDb, task: &Task) -> Result<(CompletedBy, Timestamp), StoreError> {
  if let Some(info) = db.shared_task_info(task.id())?
    && info.kept_tags.is_none()
  {
    return Ok((CompletedBy::Member, info.updated_at.unwrap_or_else(Timestamp::now)));
  }
  let closing = db
    .task_organization_history(task.id())?
    .into_iter()
    .rev()
    .find(|change| change.to.status().is_closed() && !change.from.status().is_closed());
  Ok(match closing {
    Some(change) if change.actor == ChangeActor::Orchester => (CompletedBy::Automation, change.at),
    Some(change) => (CompletedBy::Person, change.at),
    None => (CompletedBy::Person, Timestamp::now()),
  })
}

/// The last step's report of `run`, as its artifact *Report* made at `at`; `None` when no step reported anything.
fn report_artifact(db: &ProjectDb, run: &Run, at: Timestamp) -> Result<Option<RunArtifact>, StoreError> {
  let outputs = db.step_outputs(run.id())?;
  let plans = db.plans(run.id()).map_err(run_store_error)?;
  Ok(last_report(run, &outputs, &plans).map(|(step, text)| RunArtifact {
    run: run.id(),
    name: REPORT_ARTIFACT.to_owned(),
    step: Some(step.clone()),
    text: text.to_owned(),
    created_at: at,
  }))
}
