//! Removing what a run left: a person's removal from the run's dialog, or one automation asked for.

use std::{
  panic::{AssertUnwindSafe, catch_unwind},
  sync::{Arc, mpsc},
  thread,
};

use orchester_core::integration::Leftovers;
use orchester_exec::programs::Programs;

use super::{
  super::integration::{IntegrationProblem, RemovalRefused, remove_in},
  Active, JobState, RunJobs,
};
use crate::{
  activity::{Activity, operations},
  messages::{self, Message},
  workspace::OpenProject,
};

impl RunJobs {
  /// Removes what `args` lists on a thread of its own. A person's removal shows in the run's dialog
  /// and row: [`Remover::Person`]'s hook closes the dialog when it is done, and a failure stays under
  /// the row and in the status bar. A removal automation asked for ([`Remover::Automation`]) answers
  /// only its caller: the person's dialog stays open and shows the worktree removed, and a failure
  /// touches neither what the person sees nor the status bar's failures.
  pub fn start_remove(&self, args: RemoveArgs) {
    if !self.begin(Active::Remove) {
      return;
    }
    let what = operations::run_cleanup(&args.open, args.task, (&args.key, args.number));
    self.name(&args.open, &what, false);
    let automation = match &args.by {
      Remover::Person(_) => {
        self.update(|state| state.remove = JobState::Running);
        None
      }
      Remover::Automation(answer) => Some(answer.clone()),
    };
    let job = self.clone();
    if let Err(error) = thread::Builder::new()
      .name("run-cleanup".into())
      .spawn(move || job.remove_now(args))
    {
      tracing::error!(%error, "cannot start run cleanup thread");
      match automation {
        Some(answer) => answer_automation(&answer, Err(messages::interrupted().into())),
        None => self.update(|state| state.remove = JobState::Failed(messages::interrupted())),
      }
      self.finish();
    }
  }

  fn remove_now(&self, args: RemoveArgs) {
    let RemoveArgs {
      open,
      task,
      key,
      number,
      run,
      confirmed,
      programs,
      activity,
      separator,
      changed,
      by,
    } = args;
    let running = activity.start(operations::run_cleanup(&open, task, (&key, number)));
    let not_done = Message::new("statusBar.operations.runCleanup.notDone")
      .with("task", key)
      .with("run", number.to_string());
    let result = catch_unwind(AssertUnwindSafe(|| {
      remove_in(&open.repo_root, &programs, &run, &confirmed)
    }));
    let run_changed = matches!(result, Ok(Err(IntegrationProblem::RunChanged)));
    let outcome = match (result, &by) {
      (Ok(Ok(())), _) => {
        running.finish();
        Ok(())
      }
      (Ok(Err(problem)), Remover::Person(_)) => {
        let message = problem.message(&separator);
        problem.end(running, not_done);
        Err(message)
      }
      // A tool's failure is only in its result: the operation leaves the status bar without an event.
      (Ok(Err(problem)), Remover::Automation(_)) => {
        drop(running);
        Err(problem.message(&separator))
      }
      (Err(_), remover) => {
        tracing::error!(
          automation = matches!(remover, Remover::Automation(_)),
          "run cleanup thread panicked"
        );
        match remover {
          Remover::Person(_) => running.fail(messages::interrupted()),
          Remover::Automation(_) => drop(running),
        }
        Err(messages::interrupted())
      }
    };
    self.publish_removal(&outcome, &by);
    self.finish();
    changed();
    if let Remover::Automation(answer) = by {
      answer_automation(
        &answer,
        outcome.map_err(|message| RemovalRefused { message, run_changed }),
      );
    }
  }

  fn publish_removal(&self, outcome: &Result<(), Message>, by: &Remover) {
    match (outcome, by) {
      (Ok(()), Remover::Person(removed)) => {
        removed();
        self.update(|state| state.remove = JobState::Succeeded);
      }
      // The person's dialog, if open, now shows the worktree removed.
      (Ok(()), Remover::Automation(_)) => self.update(|state| state.remove = JobState::Succeeded),
      (Err(message), Remover::Person(_)) => self.update(|state| state.remove = JobState::Failed(message.clone())),
      (Err(_), Remover::Automation(_)) => {}
    }
  }

  /// Forgets a removal that failed, which stayed to be shown under the run's row: the person asks
  /// for the removal anew.
  pub fn clear_failed_removal(&self) {
    self
      .jobs
      .0
      .send_if_modified(|state| match state.runs.get_mut(&self.run) {
        Some(run) if matches!(run.remove, JobState::Failed(_)) => {
          run.remove = JobState::Idle;
          true
        }
        _ => false,
      });
  }
}

#[derive(Clone)]
pub struct RemoveArgs {
  pub open: OpenProject,
  pub task: orchester_core::id::TaskId,
  pub key: String,
  pub number: u32,
  pub run: orchester_core::run::Run,
  pub confirmed: Leftovers,
  pub programs: Programs,
  pub activity: Activity,
  pub separator: Arc<str>,
  pub changed: Arc<dyn Fn() + Send + Sync>,
  pub by: Remover,
}

/// Tells the automation that asked for a removal how it ended. A caller that stopped waiting no longer
/// needs it: the removal stands either way, as the status bar and the run's page show.
fn answer_automation(answer: &mpsc::Sender<Result<(), RemovalRefused>>, outcome: Result<(), RemovalRefused>) {
  if answer.send(outcome).is_err() {
    tracing::debug!("the automation that asked for a worktree removal stopped waiting for it");
  }
}

/// Who asked for a removal.
#[derive(Clone)]
pub enum Remover {
  /// A person, in the run's dialog; the hook runs once the worktrees are gone, before the removal
  /// is published as done, and closes that dialog.
  Person(Arc<dyn Fn() + Send + Sync>),
  /// Automation through Orchester's MCP server, which waits for the outcome on this channel.
  Automation(mpsc::Sender<Result<(), RemovalRefused>>),
}
