//! Resolving a merge's conflicts by hand in the run's worktree ([`crate::tasks::resolve`]) on a thread of its own:
//! merging the base branch in and opening the editor, committing the merge once it is resolved, or aborting it. One
//! operation of the run at a time, as for its merge; once the merge in the worktree is committed (or git merged
//! without a conflict) the merge into the base branch that conflicted is forgotten, so *Merge into main* is offered
//! again.

use std::{path::Path, sync::Arc, thread};

use orchester_core::{id::RunId, integration::resolve::Started};
use orchester_exec::programs::Programs;

use super::{Active, JobState, RunJobs};
use crate::{
  messages,
  tasks::{changes::run_worktree, resolve},
  workspace::OpenProject,
};

/// What resolving by hand does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolveStep {
  /// Merge the base branch into the worktree and open the editor.
  Start,
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
  /// Counts the operations that ended, so what the worktree holds is read again after each.
  pub ended: u64,
}

/// What an operation needs.
#[derive(Clone)]
pub struct ResolveArgs {
  pub open: OpenProject,
  pub run: RunId,
  pub programs: Programs,
  /// The editor to open the worktree in, or why none opens.
  pub editor: Result<resolve::Editor, EditorOpened>,
  /// Called once it ended, so the task is read again.
  pub changed: Arc<dyn Fn() + Send + Sync>,
}

impl RunJobs {
  /// Starts `step` on a thread of its own; whether it began, which it does not while another operation of the run
  /// works.
  pub fn start_resolving(&self, step: ResolveStep, args: ResolveArgs) -> bool {
    if !self.begin(Active::Resolve) {
      return false;
    }
    self.update(|state| {
      state.resolve.step = Some(step);
      state.resolve.state = JobState::Running;
      if step == ResolveStep::Start {
        state.resolve.editor = None;
      }
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
    let (root, run, programs) = (&args.open.repo_root, args.run, &args.programs);
    let (outcome, editor) = match step {
      ResolveStep::Start => match resolve::start(programs, root, run) {
        Ok(Started::Conflicts(files)) => (
          Ok(false),
          Some(open_editor(args.editor, &run_worktree(root, run), &files)),
        ),
        Ok(Started::Merged(_)) => (Ok(true), None),
        Err(message) => (Err(message), None),
      },
      ResolveStep::Continue => (resolve::finish(programs, root, run).map(|()| true), None),
      ResolveStep::Abort => (resolve::abort(programs, root, run).map(|()| false), None),
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

/// Opens `worktree` and the conflicted `files` in `editor`, or says why it does not open.
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
