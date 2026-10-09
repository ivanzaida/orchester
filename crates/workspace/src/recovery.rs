//! The runs the previous session left at work, for the start-up recovery (02.E `o8DCd`,
//! `docs/architecture/storage.md`, "Lifecycle"): after Quit now or a crash, every run of the
//! registered projects still recorded as running because no runner was left to record its end,
//! or recorded as interrupted by a runner of this session when it attached. A run that waits for a
//! person (a question or a plan, with no agent at work) is not interrupted: its runner leaves it
//! waiting, and the answer or the decision resumes its agent's session. Runs interrupted
//! earlier, e.g. ones not continued at an earlier start, are not this end's and are not listed.
//! Nothing continues on its own; the user continues all of them or none.
//! Blocks on SQLite and git; the UI calls it off the UI thread.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  command::Command,
  connection::AgentProvider,
  id::{ProjectId, RunId, TaskId},
  run::{InterruptCause, ListedRun, Run, RunEvent, RunStatus},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_ipc::SessionEnd;
use orchester_store::{ProjectDb, RunStoreError, StoreError};

use crate::{
  display,
  tasks::{
    changes::{GIT, run_worktree},
    detail,
  },
  workspace::Workspace,
};

/// A run the previous session left interrupted, as the recovery table lists it (`KzyXA`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterruptedRun {
  pub project_id: ProjectId,
  pub project_root: PathBuf,
  pub project: Arc<str>,
  pub run: RunId,
  pub task: TaskId,
  pub task_key: Arc<str>,
  pub task_title: Arc<str>,
  pub number: u32,
  /// The step it was working on, with its agent role, provider and model.
  pub step: Option<Arc<str>>,
  pub agent: Option<Arc<str>>,
  pub provider: Option<AgentProvider>,
  pub model: Option<Arc<str>>,
  pub cause: InterruptCause,
  /// Its last checkpoint: the short revision and when it was saved.
  pub checkpoint: Option<(Arc<str>, Timestamp)>,
  pub worktree: WorktreeState,
}

/// What the run's worktree holds beyond its last checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorktreeState {
  Clean,
  /// This many files are not committed; they are kept, never discarded silently.
  Uncommitted(usize),
  /// git could not tell (the worktree is gone, or git is missing).
  Unknown,
}

/// Whether the previous session's end is one the recovery reports: Quit now or a crash. After
/// "Wait for agents to save" the runs are simply paused.
pub fn reports(previous: SessionEnd) -> bool {
  matches!(previous, SessionEnd::QuitNow | SessionEnd::Crashed)
}

/// The runs `previous`, the end of the last session, left interrupted in the registered
/// projects; this session started at `since`. A project whose database cannot be read is
/// skipped and logged; it shows as unavailable when opened, and its runner records the
/// interruption when it starts.
pub fn interrupted_runs(
  workspace: &Workspace,
  programs: &Programs,
  previous: SessionEnd,
  since: Timestamp,
) -> Result<Vec<InterruptedRun>, StoreError> {
  let stale_cause = orchester_ipc::AttachSession::Fresh { previous_end: previous }.interrupt_cause();
  let mut found = Vec::new();
  for project in workspace.projects()? {
    let db = match ProjectDb::open(&project.repo_root) {
      Ok(db) => db,
      Err(error) => {
        tracing::warn!(project = %project.repo_root.display(), %error, "cannot read a project for recovery");
        continue;
      }
    };
    let name: Arc<str> = project.name.as_str().into();
    for run in left_at_work(&db, since)? {
      let cause = match run.status() {
        RunStatus::Interrupted(cause) => *cause,
        _ => stale_cause,
      };
      found.push(describe(&db, programs, (&project.repo_root, &name), run, cause)?);
    }
  }
  Ok(found)
}

/// Runs still recorded as running with an agent or a step at work, as the runner interrupts them
/// when it attaches, and runs recorded as interrupted since `since` (by this session's runner when
/// it attached), ordered by number. A run that cannot be read cannot be continued: it is logged and
/// left out, and its task shows it as unreadable.
fn left_at_work(db: &ProjectDb, since: Timestamp) -> Result<Vec<Run>, StoreError> {
  let mut runs: Vec<Run> = readable(db.running_runs().map_err(detail::run_store_error)?)
    .into_iter()
    .filter(|run| !run.waits_for_a_person())
    .collect();
  for run in readable(db.interrupted_runs().map_err(detail::run_store_error)?) {
    let interrupted_at = db
      .events(run.id())
      .map_err(detail::run_store_error)?
      .last()
      .map(RunEvent::at);
    if interrupted_at.is_some_and(|at| at >= since) {
      runs.push(run);
    }
  }
  runs.sort_by_key(|run| run.number());
  Ok(runs)
}

/// The runs of `listed` that can be read; the others are logged.
fn readable(listed: Vec<ListedRun<RunStoreError>>) -> Vec<Run> {
  listed
    .into_iter()
    .filter_map(|run| match run {
      Ok(run) => Some(run),
      Err(unreadable) => {
        tracing::warn!(run = %unreadable.id, error = %unreadable.error, "a run left at work cannot be read");
        None
      }
    })
    .collect()
}

fn describe(
  db: &ProjectDb,
  programs: &Programs,
  (root, project): (&Path, &Arc<str>),
  run: Run,
  cause: InterruptCause,
) -> Result<InterruptedRun, StoreError> {
  let task = db.task(run.task())?.ok_or(StoreError::TaskNotFound(run.task()))?;
  let view = detail::run_view(db, run)?;
  let step = view.current_step();
  let checkpoint = view.run.last_checkpoint().map(|checkpoint| {
    (
      Arc::<str>::from(display::short_revision(checkpoint.revision().as_str())),
      checkpoint.created_at(),
    )
  });
  let worktree = run_worktree(root, view.run.id());
  Ok(InterruptedRun {
    project_id: db.project().id(),
    project_root: root.to_path_buf(),
    project: project.clone(),
    run: view.run.id(),
    task: task.id(),
    task_key: task.key().to_string().into(),
    task_title: task.details().title.as_str().into(),
    number: view.run.number().get(),
    step: step.map(|step| step.name.clone()),
    agent: step.and_then(|step| step.agent.clone()),
    provider: step.and_then(|step| step.provider),
    model: step.and_then(|step| step.model.clone()),
    cause,
    checkpoint,
    worktree: worktree_state(programs, &worktree),
  })
}

/// Counts the files git reports as changed or untracked in `worktree`.
fn worktree_state(programs: &Programs, worktree: &Path) -> WorktreeState {
  if !worktree.is_dir() {
    return WorktreeState::Unknown;
  }
  let status =
    Command::new(Path::new(GIT), ["status", "--porcelain", "--untracked-files=all"]).with_current_dir(worktree);
  match programs.runner().run(&status) {
    Ok(output) if output.success() => match output.stdout.lines().filter(|line| !line.trim().is_empty()).count() {
      0 => WorktreeState::Clean,
      files => WorktreeState::Uncommitted(files),
    },
    Ok(output) => {
      tracing::warn!(worktree = %worktree.display(), stderr = %output.stderr, "git status failed");
      WorktreeState::Unknown
    }
    Err(error) => {
      tracing::warn!(worktree = %worktree.display(), %error, "cannot run git status");
      WorktreeState::Unknown
    }
  }
}
