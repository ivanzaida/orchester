//! The runs the previous session left at work, for the start-up recovery (02.E `o8DCd`,
//! `docs/architecture/storage.md`, "Lifecycle"): after Quit now or a crash, every run of the
//! registered projects still recorded as running because no runner was left to record its end,
//! or recorded as interrupted by a runner of this session when it attached. A run that waits for a
//! person (a question or a plan, with no agent at work) is not interrupted: its runner leaves it
//! waiting, and the answer or the decision resumes its agent's session. Runs interrupted
//! earlier, e.g. ones not continued at an earlier start, are not this end's and are not listed.
//! Nothing continues on its own; the user continues all of them or none.
//! Blocks on SQLite and git; the UI calls it off the UI thread. Runs or projects that cannot be read are
//! named, and the others are still offered. [`Recovery`] reads them once for the
//! session and keeps what the person decided.

mod pending;
mod session;

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  command::Command,
  connection::AgentProvider,
  id::{ProjectId, RunId, TaskId},
  project::TaskKey,
  run::{InterruptCause, ListedRun, Run, RunEvent, RunStatus},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_ipc::SessionEnd;
use orchester_store::{ProjectDb, RunStoreError, StoreError};

pub use self::session::{Continuation, Decision, NotContinued, Reading, Recovery, RecoveryState};
use crate::{
  display,
  messages::{self, Message},
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
  pub task_key: TaskKey,
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
  /// The first file of the worktree that is not committed, which *View changes* opens.
  pub uncommitted_file: Option<Arc<str>>,
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

/// What the recovery found: the runs it can offer, and those it could not read, by project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
  pub runs: Vec<InterruptedRun>,
  pub unread: Vec<Unread>,
  /// The projects whose database could not be opened; they show as unavailable when opened.
  pub unopened: Vec<ProjectId>,
}

/// A run left at work that cannot be read, or a project whose runs cannot be: it stays as it is, and
/// its task page shows it; the recovery names it rather than leaving it out silently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unread {
  pub project_id: ProjectId,
  /// The run, when it is one run; `None` when the project's runs could not be listed.
  pub run: Option<RunId>,
  /// The run's task, when it could be read.
  pub task_key: Option<TaskKey>,
  pub reason: Message,
}

/// The runs `previous`, the end of the last session, left interrupted in the registered projects, each
/// project's interrupted at what `since` says for it or later (the previous session's start: a runner
/// can record its runs as interrupted before this session starts), or none of it when it says `None`. A
/// project whose database cannot be opened is skipped and logged; it shows as unavailable when opened,
/// and its runner records the interruption when it starts. What cannot be read is named in
/// [`Found::unread`], and the rest is still offered.
pub fn interrupted_runs(
  workspace: &Workspace,
  programs: &Programs,
  previous: SessionEnd,
  since: &dyn Fn(ProjectId) -> Option<Timestamp>,
) -> Result<Found, StoreError> {
  let stale_cause = orchester_ipc::AttachSession::Fresh { previous_end: previous }.interrupt_cause();
  let mut found = Found::default();
  for project in workspace.projects()? {
    let Some(since) = since(project.id) else {
      continue;
    };
    let db = match ProjectDb::open(&project.repo_root) {
      Ok(db) => db,
      Err(error) => {
        tracing::warn!(project = %project.repo_root.display(), %error, "cannot read a project for recovery");
        found.unopened.push(project.id);
        continue;
      }
    };
    let project_id = db.project().id();
    let unread = |run: Option<(RunId, TaskId)>, error: &StoreError| Unread {
      project_id,
      run: run.map(|(run, _)| run),
      task_key: run.and_then(|(_, task)| task_key(&db, task)),
      reason: messages::store_error(error),
    };
    let (runs, unreadable) = match left_at_work(&db, since) {
      Ok(left) => left,
      Err(error) => {
        tracing::warn!(project = %project.repo_root.display(), %error, "cannot list a project's runs for recovery");
        found.unread.push(unread(None, &error));
        continue;
      }
    };
    found.unread.extend(
      unreadable
        .iter()
        .map(|(run, task, error)| unread(Some((*run, *task)), error)),
    );
    let name: Arc<str> = project.name.as_str().into();
    for run in runs {
      let (id, task) = (run.id(), run.task());
      let cause = match run.status() {
        RunStatus::Interrupted(cause) => *cause,
        _ => stale_cause,
      };
      match describe(&db, programs, (&project.repo_root, &name), run, cause) {
        Ok(interrupted) => found.runs.push(interrupted),
        Err(error) => {
          tracing::warn!(run = %id, %error, "cannot describe an interrupted run");
          found.unread.push(unread(Some((id, task)), &error));
        }
      }
    }
  }
  Ok(found)
}

/// A run that cannot be read, its task, and why.
type Unreadable = (RunId, TaskId, StoreError);

/// The key of `task`, to name a run that cannot be read; `None` when the task cannot be read either, and
/// the run is named without it.
fn task_key(db: &ProjectDb, task: TaskId) -> Option<TaskKey> {
  match db.task(task) {
    Ok(task) => task.map(|task| task.key().clone()),
    Err(error) => {
      tracing::warn!(%task, %error, "cannot read the task of a run that cannot be read");
      None
    }
  }
}

/// Runs still recorded as running with an agent or a step at work, as the runner interrupts them
/// when it attaches, and runs recorded as interrupted since `since`, ordered by number; and the runs
/// recorded as running that cannot be read, with why. An unreadable run recorded as interrupted is
/// left out and logged: when it was interrupted cannot be read either.
fn left_at_work(db: &ProjectDb, since: Timestamp) -> Result<(Vec<Run>, Vec<Unreadable>), StoreError> {
  let (running, unreadable) = readable(db.running_runs().map_err(detail::run_store_error)?);
  let mut runs: Vec<Run> = running.into_iter().filter(|run| !run.waits_for_a_person()).collect();
  for run in readable(db.interrupted_runs().map_err(detail::run_store_error)?).0 {
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
  Ok((runs, unreadable))
}

/// The runs of `listed` that can be read, and the others with why; those are logged.
fn readable(listed: Vec<ListedRun<RunStoreError>>) -> (Vec<Run>, Vec<Unreadable>) {
  let mut runs = Vec::new();
  let mut unreadable = Vec::new();
  for run in listed {
    match run {
      Ok(run) => runs.push(run),
      Err(run) => {
        tracing::warn!(run = %run.id, error = %run.error, "a run left at work cannot be read");
        unreadable.push((run.id, run.task, detail::run_store_error(run.error)));
      }
    }
  }
  (runs, unreadable)
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
  let (state, uncommitted_file) = worktree_state(programs, &worktree);
  Ok(InterruptedRun {
    project_id: db.project().id(),
    project_root: root.to_path_buf(),
    project: project.clone(),
    run: view.run.id(),
    task: task.id(),
    task_key: task.key().clone(),
    task_title: task.details().title.as_str().into(),
    number: view.run.number().get(),
    step: step.map(|step| step.name.clone()),
    agent: step.and_then(|step| step.agent.clone()),
    provider: step.and_then(|step| step.provider),
    model: step.and_then(|step| step.model.clone()),
    cause,
    checkpoint,
    worktree: state,
    uncommitted_file,
  })
}

/// Counts the files git reports as changed or untracked in `worktree`, and names the first of them as a
/// repository path.
fn worktree_state(programs: &Programs, worktree: &Path) -> (WorktreeState, Option<Arc<str>>) {
  if !worktree.is_dir() {
    return (WorktreeState::Unknown, None);
  }
  let status =
    Command::new(Path::new(GIT), ["status", "--porcelain", "--untracked-files=all"]).with_current_dir(worktree);
  match programs.runner().run(&status) {
    Ok(output) if output.success() => {
      let files: Vec<&str> = output.stdout.lines().filter(|line| !line.trim().is_empty()).collect();
      // `XY path`, or `XY from -> to` for a rename: the path the worktree has now.
      let first = files.first().and_then(|line| line.get(3..)).map(|path| {
        let path = path.rsplit(" -> ").next().unwrap_or(path);
        Arc::from(path.trim_matches('"'))
      });
      match files.len() {
        0 => (WorktreeState::Clean, None),
        count => (WorktreeState::Uncommitted(count), first),
      }
    }
    Ok(output) => {
      tracing::warn!(worktree = %worktree.display(), stderr = %output.stderr, "git status failed");
      (WorktreeState::Unknown, None)
    }
    Err(error) => {
      tracing::warn!(worktree = %worktree.display(), %error, "cannot run git status");
      (WorktreeState::Unknown, None)
    }
  }
}
