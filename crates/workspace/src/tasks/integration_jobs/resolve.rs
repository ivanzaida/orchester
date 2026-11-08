//! Resolving a merge's conflicts by hand in the run's worktree ([`crate::tasks::resolve`]) on a thread of its own:
//! merging the base branch in and opening the editor, keeping a side of a file git could not merge, committing the
//! merge once it is resolved, or aborting it. One
//! operation of the run at a time, as for its merge; once the merge in the worktree is committed (or git merged
//! without a conflict) the merge into the base branch that conflicted is forgotten, so *Merge into main* is offered
//! again.

use std::{path::Path, sync::Arc, thread};

use orchester_core::{
  id::RunId,
  integration::resolve::{Side, Started},
};

use super::{Active, JobState, RunJobs};
use crate::{
  messages,
  tasks::{changes::run_worktree, resolve},
  workspace::OpenProject,
};

/// What resolving by hand does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolveStep {
  /// Merge the base branch into the worktree and open the editor.
  Start,
  /// Keep `side`'s version of `path`, a file git could not merge by content.
  Decide { path: Arc<str>, side: Side },
  /// Commit the resolved merge.
  Continue,
  /// Abort the merge.
  Abort,
}

/// How the editor went after the merge stopped at its conflicts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorOpened {
  Opened(resolve::Editor),
  /// No editor was found.
  NoEditor,
  /// Automation pressed it, and only a person starts an editor they did not name for it.
  PersonOnly,
  /// The editor did not start; why, for the log.
  Failed(Arc<str>),
}

/// A resolution operation's state, as the task page shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolveJob {
  /// The last operation asked for.
  pub step: Option<ResolveStep>,
  pub state: JobState,
  /// After the merge stopped at conflicts: whether the editor opened.
  pub editor: Option<EditorOpened>,
  /// The base branch merged into the worktree without a conflict, or was in the run's branch already: there was
  /// nothing to open an editor on.
  pub nothing_to_resolve: bool,
  /// Counts the operations that ended, so what the worktree holds is read again after each.
  pub ended: u64,
}

/// What an operation needs.
#[derive(Clone)]
pub struct ResolveArgs {
  pub open: OpenProject,
  pub run: RunId,
  /// The project's run worktrees, where git runs.
  pub worktrees: resolve::RunWorktrees,
  /// The editor to open the worktree in, or why none opens.
  pub editor: Result<resolve::Editor, EditorOpened>,
  /// Called once it ended, so the task is read again.
  pub changed: Arc<dyn Fn() + Send + Sync>,
}

impl RunJobs {
  /// Forgets the merge into the base branch that failed: the run's work went back to a step that resolves it, so
  /// what that merge found no longer stands for the run.
  pub fn forget_merge_failure(&self) {
    self.update(|state| {
      if matches!(state.merge, JobState::Failed(_)) {
        state.merge = JobState::Idle;
        state.conflicts.clear();
        state.continues_locally = false;
      }
    });
  }

  /// Starts `step` on a thread of its own; whether it began, which it does not while another operation of the run
  /// works.
  pub fn start_resolving(&self, step: ResolveStep, args: ResolveArgs) -> bool {
    if !self.begin(Active::Resolve) {
      return false;
    }
    self.update(|state| {
      if step == ResolveStep::Start {
        state.resolve.editor = None;
        state.resolve.nothing_to_resolve = false;
      }
      state.resolve.step = Some(step.clone());
      state.resolve.state = JobState::Running;
    });
    let job = self.clone();
    if let Err(error) = thread::Builder::new()
      .name("run-resolve".into())
      .spawn(move || job.resolve_now(step, args))
    {
      tracing::error!(%error, "cannot start the thread that resolves a run's merge");
      self.update(|state| state.resolve.state = JobState::Failed(messages::interrupted()));
      self.finish();
    }
    true
  }

  fn resolve_now(&self, step: ResolveStep, args: ResolveArgs) {
    let (root, run, worktrees) = (&args.open.repo_root, args.run, &args.worktrees);
    let starting = step == ResolveStep::Start;
    let (outcome, editor) = match step {
      ResolveStep::Start => match resolve::start(worktrees, root, run) {
        Ok(Started::Conflicts(files)) => (
          Ok(false),
          Some(open_editor(args.editor, &run_worktree(root, run), &files)),
        ),
        Ok(Started::Merged(_)) => (Ok(true), None),
        Err(message) => (Err(message), None),
      },
      ResolveStep::Decide { path, side } => (
        resolve::decide(worktrees, root, (run, &path, side)).map(|()| false),
        None,
      ),
      ResolveStep::Continue => (resolve::finish(worktrees, root, run).map(|()| true), None),
      ResolveStep::Abort => (resolve::abort(worktrees, root, run).map(|()| false), None),
    };
    self.update(|state| {
      state.resolve.ended += 1;
      if editor.is_some() {
        state.resolve.editor = editor;
      }
      match outcome {
        // The run's branch holds the base branch now: the merge into it that conflicted is behind.
        Ok(true) => {
          state.resolve.state = JobState::Succeeded;
          // Merged from *Resolve in editor*: nothing conflicted, so no editor opened and the page says so.
          state.resolve.nothing_to_resolve = starting;
          state.merge = JobState::Idle;
          state.conflicts.clear();
          state.continues_locally = false;
        }
        Ok(false) => state.resolve.state = JobState::Succeeded,
        Err(message) => state.resolve.state = JobState::Failed(message),
      }
    });
    self.finish();
    (args.changed)();
  }
}

/// Opens `worktree` and the conflicted `files` in `editor`, or says why it does not open. The editor runs on this
/// computer and opens the worktree's local path, where a local runner keeps it; a run whose worktree is on another
/// computer (`docs/architecture/remote-runners.md`) will need another way to reach its files, which is not built.
fn open_editor(editor: Result<resolve::Editor, EditorOpened>, worktree: &Path, files: &[String]) -> EditorOpened {
  let editor = match editor {
    Ok(editor) => editor,
    Err(why) => return why,
  };
  match editor.open(worktree, files) {
    Ok(()) => EditorOpened::Opened(editor),
    Err(error) => {
      tracing::warn!(%error, worktree = %worktree.display(), "the editor did not start");
      EditorOpened::Failed(error.to_string().into())
    }
  }
}
