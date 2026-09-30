//! Run again (ORC-16 `zM693`, `R5y5x`, `iE8eQ`) as the task page shows it: whether the task whose
//! latest run failed or was cancelled can run again now, by core's rule
//! ([`orchester_core::run::run_again`]), and what its next run would use, for the confirmation.
//! The runner checks the rule again when it is asked. Blocks on SQLite and git; the UI calls it
//! off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  command::CommandRunner,
  diff::git::git_command,
  id::{RunId, TaskId},
  integration::checked_out_branch,
  resource::{Pipeline, VersionNumber},
  revision::Revision,
  run::{BranchName, Run, RunAgainBlock, RunAgainInput, RunNumber, run_again},
  store::{ActivityStore, ResourceStore, RunStore, SettingsStore, TaskStore},
  task::TaskLimits,
  time::Timestamp,
  tokens::compact_tokens,
};
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, StoreError};

use super::{changes::GIT, detail::run_store_error, run::start_messages};
use crate::{messages::Message, programs::Programs, runner::RunRequestError};

/// Where the next run starts from: the branch the user's checkout has, if any, at its `HEAD`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
  pub branch: Option<BranchName>,
  pub revision: Revision,
}

/// What the task's next run uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextRun {
  pub number: RunNumber,
  /// The latest published version of the task's pipeline.
  pub version: VersionNumber,
  /// The pipeline's first step, which the run starts from, and how many steps it has.
  pub first_step: Arc<str>,
  pub steps: usize,
  /// `None` when git could not tell; the runner reads it again when the run starts.
  pub base: Option<Base>,
  pub branch: BranchName,
  pub limits: TaskLimits,
}

/// Whether the task runs again after its latest run `after`, and with what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunAgainCheck {
  pub after: RunId,
  /// The name of the task's pipeline.
  pub pipeline: Arc<str>,
  pub next: Result<NextRun, RunAgainBlock>,
}

impl lurq::app::component::DevtoolsInspectable for RunAgainCheck {}

impl RunAgainCheck {
  /// Checks task `task` of the project at `repo_root` after its latest run `after`.
  pub fn load_in(repo_root: &Path, programs: &Programs, task: TaskId, after: RunId) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let task = db.task(task)?.ok_or(StoreError::TaskNotFound(task))?;
    let listed = db.runs_of_task(task.id()).map_err(run_store_error)?;
    let runs: Vec<Option<&Run>> = listed.iter().map(|run| run.as_ref().ok()).collect();
    let (name, latest) = match task.details().pipeline {
      Some(pipeline) => (
        db.resource(pipeline)?
          .map_or_else(|| Arc::from(""), |resource| resource.name().as_str().into()),
        db.latest_version::<Pipeline>(pipeline)?,
      ),
      None => (Arc::from(""), None),
    };
    let input = RunAgainInput {
      runs: &runs,
      after,
      pipeline_published: latest.is_some(),
      limits: db.project_limits()?,
      used_today: db.tokens_used_since(Timestamp::now().local_day_start())?,
    };
    if !db.unfinished_blocking_tasks(task.id())?.is_empty() {
      return Ok(Self {
        after,
        pipeline: name,
        next: Err(RunAgainBlock::TaskBlocked),
      });
    }
    if let Err(block) = run_again(&input) {
      return Ok(Self {
        after,
        pipeline: name,
        next: Err(block),
      });
    }
    let version = latest.expect("the rule needs a published pipeline");
    let steps = version.body().steps();
    let next = NextRun {
      number: db.next_run_number().map_err(run_store_error)?,
      version: version.number(),
      first_step: steps
        .first()
        .map_or_else(|| Arc::from(""), |step| step.name.as_str().into()),
      steps: steps.len(),
      base: base(programs.runner(), repo_root),
      branch: BranchName::for_next_run(task.key(), listed.len()),
      limits: task.details().limits.expect("a task that ran has limits"),
    };
    Ok(Self {
      after,
      pipeline: name,
      next: Ok(next),
    })
  }
}

/// Why the task of pipeline `pipeline` cannot run again, as the lock line under the run's notice
/// says it (`iE8eQ`).
pub fn block_message(block: RunAgainBlock, pipeline: &str) -> Message {
  match block {
    RunAgainBlock::TaskBlocked => Message::new("tasks.blocked.runUnavailable"),
    RunAgainBlock::NotEnded => Message::new("runs.runAgain.unavailable.notEnded"),
    RunAgainBlock::RunActive { number } => {
      Message::new("runs.runAgain.unavailable.runActive").with("run", number.get().to_string())
    }
    RunAgainBlock::PipelineNotPublished => {
      Message::new("runs.runAgain.unavailable.pipelineNotPublished").with("pipeline", pipeline.to_owned())
    }
    RunAgainBlock::DailyBudgetUsed { used, budget } => Message::new("runs.runAgain.unavailable.dailyBudgetUsed")
      .with("used", compact_tokens(used.get()))
      .with("budget", compact_tokens(budget.get())),
  }
}

/// What a person reads when the runner did not run the task again.
pub fn request_messages(error: &RunRequestError, pipeline: &str) -> Vec<Message> {
  match error {
    RunRequestError::Rejected(Rejection::RunAgainBlocked { block }) => vec![block_message(*block, pipeline)],
    RunRequestError::Rejected(Rejection::WorktreeChanged { path }) => {
      vec![Message::new("runs.worktree.failure.newChanges").with("path", path.clone())]
    }
    RunRequestError::Rejected(Rejection::WorktreeNotRemoved { path, detail }) => vec![
      Message::new("runs.worktree.failure.notRemoved")
        .with("path", path.clone())
        .with("detail", detail.clone()),
    ],
    other => start_messages(other),
  }
}

/// The checkout's branch and `HEAD` at `repo_root`; `None`, logged, when git cannot tell.
fn base(runner: &(dyn CommandRunner + Send + Sync), repo_root: &Path) -> Option<Base> {
  let git = Path::new(GIT);
  let head = runner.run(&git_command(
    git,
    repo_root,
    ["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
  ));
  let revision = match head {
    Ok(output) if output.success() => Revision::new(output.stdout.trim()).ok(),
    Ok(output) => {
      tracing::warn!(stderr = %output.stderr, "git did not name the checkout's HEAD");
      None
    }
    Err(error) => {
      tracing::warn!(%error, "cannot run git for the checkout's HEAD");
      None
    }
  }?;
  let branch = checked_out_branch(&runner, git, repo_root)
    .map_err(|error| tracing::warn!(%error, "cannot read the checkout's branch"))
    .ok()?;
  Some(Base { branch, revision })
}
