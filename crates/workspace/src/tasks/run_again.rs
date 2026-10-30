//! Run again (ORC-16 `zM693`, `R5y5x`, `iE8eQ`) as the task page shows it: whether the task whose
//! latest run failed, was cancelled or was interrupted and lost its worktree (ORC-63), or passed every step committing
//! nothing its pipeline expected (ORC-93), can run again now, by core's rule
//! ([`orchester_core::run::run_again`]), and what its next run would use, for the confirmation.
//! The runner checks the rule again when it is asked. Blocks on SQLite and git; the UI calls it
//! off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  command::CommandRunner,
  diff::git::git_command,
  id::{RunId, TaskId},
  integration::{
    base::{BaseChoice, set_branch_tip},
    checked_out_branch,
  },
  resource::{Pipeline, VersionNumber, WithoutCommits},
  revision::Revision,
  run::{BranchName, Run, RunAgainBlock, RunAgainInput, RunNumber, RunStatus, run_again, run_delivers},
  store::{ActivityStore, ResourceStore, RunStore, SettingsStore, TaskStore, WorktreeStore},
  task::TaskLimits,
  time::Timestamp,
  tokens::compact_tokens,
};
use orchester_exec::programs::Programs;
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, StoreError};

use super::{
  changes::GIT,
  delivery::{self, Delivery},
  detail::run_store_error,
  run::start_messages,
};
use crate::{messages::Message, runner::RunRequestError};

/// Where the next run starts from: the task's or its module's base branch at its tip, or the branch the user's
/// checkout has, if any, at its `HEAD`.
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
    let worktree_removed: Vec<RunId> = db
      .worktree_removals(task.id())?
      .into_iter()
      .map(|(run, _)| run)
      .collect();
    // A run that committed nothing its pipeline expected runs again too (ORC-93), as git tells.
    let committed_nothing = runs.last().copied().flatten().is_some_and(|run| {
      run.id() == after
        && *run.status() == RunStatus::Succeeded
        && run_delivers(&task, run).without_commits() == WithoutCommits::Flagged
        && delivery::of_run(programs, repo_root, run) == Delivery::NothingToMerge
    });
    let input = RunAgainInput {
      runs: &runs,
      after,
      worktree_removed: &worktree_removed,
      pipeline_published: latest.is_some(),
      limits: db.project_limits()?,
      used_today: db.tokens_used_since(Timestamp::now().local_day_start())?,
      committed_nothing,
    };
    // A run that has not ended says so first; a blocked task then says what blocks it before the
    // pipeline or the day's budget.
    let block = match run_again(&input) {
      Err(block @ (RunAgainBlock::RunActive { .. } | RunAgainBlock::NotEnded)) => Some(block),
      _ if !db.unfinished_blocking_tasks(task.id())?.is_empty() => Some(RunAgainBlock::TaskBlocked),
      outcome => outcome.err(),
    };
    if let Some(block) = block {
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
      base: base(programs.runner(), repo_root, &db.next_run_base(&task)?.0),
      branch: BranchName::for_next_run(task.key(), listed.len()),
      limits: task.details().limits,
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

/// Where the next run starts at `repo_root`: the tip of the base branch `choice` sets, else the checkout's branch and
/// `HEAD`; `None`, logged, when git cannot tell or the branch set is gone.
fn base(runner: &(dyn CommandRunner + Send + Sync), repo_root: &Path, choice: &BaseChoice) -> Option<Base> {
  let git = Path::new(GIT);
  if let Some(branch) = choice.branch() {
    return match set_branch_tip(&runner, git, (repo_root, branch)) {
      Ok(revision) => Some(Base {
        branch: Some(branch.clone()),
        revision,
      }),
      Err(error) => {
        tracing::warn!(%error, "cannot read the task's base branch");
        None
      }
    };
  }
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
