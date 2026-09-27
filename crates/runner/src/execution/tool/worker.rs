//! Following one command tool's program while it runs: its output is kept up to the tool's output
//! limit, it is ended with everything it started when its timeout passes or the run ends it, and
//! once it has ended its end is recorded in the transcript and on its invocation. A command has no
//! turns: a run limit reached by an agent's turn lets it run to its end, as a pause does.

use std::{path::PathBuf, time::Instant};

use orchester_core::{
  connection::stream::AgentEvent,
  resource::{CommandAction, StepKey},
  run::{CapturedOutput, CommandEnd, Invocation, OutputCapture, StepAt, StepFailure, StepOutput, ToolCommand},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::{ProjectDb, StoreError};
use tokio::{sync::mpsc, time::sleep};

use super::super::{
  Notifier,
  step::worker::{Stop, StopCause, WorkerEvent},
};
use crate::supervisor::{ProcessEvent, ProcessExit, Supervised};

/// How a command ended, for its run.
pub(in super::super) struct CommandFinished {
  /// Why the run ended it; `None` when it ended on its own or ran out of time.
  pub(in super::super) stopped: Option<StopCause>,
  /// Why its step fails; `None` when it passes or the run ended it.
  pub(in super::super) failure: Option<StepFailure>,
  /// What its step leaves for the steps after it; `None` when the run ended it.
  pub(in super::super) output: Option<StepOutput>,
}

/// One running program and where its news goes.
pub(super) struct CommandWorker {
  pub(super) root: PathBuf,
  pub(super) invocation: Invocation,
  pub(super) tool: String,
  pub(super) action: CommandAction,
  pub(super) step: StepKey,
  pub(super) process: Supervised,
  pub(super) notifier: Notifier,
  pub(super) events: mpsc::UnboundedSender<WorkerEvent>,
  pub(super) stops: mpsc::UnboundedReceiver<Stop>,
}

impl CommandWorker {
  /// Follows the program until it has ended, then tells the run how.
  pub(super) async fn work(self) {
    let execution = self.invocation.execution;
    let events = self.events.clone();
    let end = self.follow().await;
    // The run's task outlives its commands; if it is gone, the runner is ending anyway.
    if events.send(WorkerEvent::CommandEnded { execution, end }).is_err() {
      tracing::debug!(%execution, "the run no longer waits for its command");
    }
  }

  async fn follow(mut self) -> Result<CommandFinished, StoreError> {
    let db = ProjectDb::open(&self.root)?;
    let started = Instant::now();
    let timeout = sleep(self.action.timeout().duration());
    tokio::pin!(timeout);
    let mut capture = OutputCapture::new(self.action.output_limit());
    let mut stopped = None;
    let mut timed_out = false;
    let mut listening = true;
    let exit = loop {
      tokio::select! {
        event = self.process.next() => match event {
          Some(ProcessEvent::Output { line, .. }) => capture.push(&line),
          Some(ProcessEvent::Exited(exit)) => break exit,
          None => break ProcessExit::WaitFailed { reason: String::new() },
        },
        () = &mut timeout, if !timed_out && stopped.is_none() => {
          timed_out = true;
          self.process.terminate();
        }
        stop = self.stops.recv(), if listening => match stop {
          Some(Stop::Now(cause)) => {
            if stopped.is_none() && !timed_out {
              stopped = Some(cause);
              self.process.terminate();
            }
          }
          // A command has no turns and runs no commands of its own: it runs to its end.
          Some(Stop::AfterTurn(_) | Stop::AfterCommand(_) | Stop::Withdraw) => {}
          None => listening = false,
        },
      }
    };
    let end = match (stopped, timed_out, exit) {
      (Some(_), ..) => CommandEnd::Stopped,
      (None, true, _) => CommandEnd::TimedOut,
      (None, false, ProcessExit::Exited { code }) => CommandEnd::Exited { code },
      (None, false, ProcessExit::Terminated | ProcessExit::WaitFailed { .. }) => CommandEnd::Exited { code: None },
    };
    let elapsed = started.elapsed();
    self.finish(db, (end, stopped), &capture.finish(), elapsed)
  }

  /// Records the end in the transcript and on the invocation, and what it means for the step.
  fn finish(
    &self,
    mut db: ProjectDb,
    (end, stopped): (CommandEnd, Option<StopCause>),
    output: &CapturedOutput,
    elapsed: std::time::Duration,
  ) -> Result<CommandFinished, StoreError> {
    let command = ToolCommand {
      tool: &self.tool,
      action: &self.action,
    };
    let now = Timestamp::now();
    let event: AgentEvent = command.finished(end, output);
    let sequence = db.append_activity(&self.invocation, &[(now, event)])?;
    self.notifier.send(Event::AgentActivity {
      run: self.invocation.run,
      sequence,
    });
    let mut invocation = self.invocation.clone();
    invocation.ended_at = Some(now);
    invocation.outcome = Some(command.outcome(end));
    db.save_invocation(&invocation)?;
    let at = StepAt {
      run: invocation.run,
      execution: invocation.execution,
      step: self.step.clone(),
      now,
    };
    Ok(CommandFinished {
      stopped,
      failure: command.failure(end),
      output: command.output(at, end, elapsed, output),
    })
  }
}
