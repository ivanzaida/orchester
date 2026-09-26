//! Following one agent CLI while it works, beside the other agents of its run: its output is
//! recorded as it arrives, the usage of each turn is handed to the run, which decides about limits
//! and budgets, and the agent is ended when the run says so. A run that stops at a token limit or
//! a step budget lets the turn that used it end: the agent has [`TURN_GRACE`] to exit on its own,
//! and starting another turn ends it at once.

use std::{path::PathBuf, time::Duration};

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome, InvocationTracker, parse_codex_line},
  id::StepExecutionId,
  run::{Invocation, RunLimit},
  store::ActivityStore,
  time::Timestamp,
  tokens::TokensUsed,
};
use orchester_ipc::Event;
use orchester_store::{ProjectDb, StoreError};
use tokio::{
  sync::mpsc,
  time::{Instant, sleep},
};

use super::super::Notifier;
use crate::supervisor::{ProcessEvent, ProcessExit, Supervised};

/// Transcript entries are written at least this often while the CLI prints…
const FLUSH_INTERVAL: Duration = Duration::from_millis(250);
/// …or as soon as this many are waiting.
const FLUSH_BATCH: usize = 64;
/// How long an agent whose turn used up a budget may take to end on its own: usage arrives when a
/// turn completes, and the CLI usually exits right after.
const TURN_GRACE: Duration = Duration::from_secs(5);

/// Why the run ends an agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum StopCause {
  /// A cancel or the runner stopping; carried out once every agent of the run has ended.
  Requested,
  Limit(RunLimit),
  /// The execution used its step budget.
  StepBudget,
  /// Another step failed and its attempt ended; this execution was skipped.
  Abandoned,
}

/// What the run tells an agent.
#[derive(Clone, Copy, Debug)]
pub(in super::super) enum Stop {
  /// End it now.
  Now(StopCause),
  /// Its turn used up a budget: end it once the turn is over.
  AfterTurn(StopCause),
}

/// What an agent tells its run.
pub(in super::super) enum WorkerEvent {
  /// A turn completed with this many counted tokens.
  Usage {
    execution: StepExecutionId,
    tokens: TokensUsed,
  },
  /// The CLI exited; `Err` when recording what it did failed.
  Ended {
    execution: StepExecutionId,
    end: Result<Finished, StoreError>,
  },
}

/// How an invocation ended.
pub(in super::super) struct Finished {
  pub(in super::super) outcome: InvocationOutcome,
  /// Why the run ended the agent; `None` when it exited on its own.
  pub(in super::super) stopped: Option<StopCause>,
  /// The last message the agent wrote.
  pub(in super::super) final_message: Option<String>,
}

/// One running agent CLI and where its news goes.
pub(in super::super) struct Worker {
  pub(in super::super) root: PathBuf,
  pub(in super::super) invocation: Invocation,
  pub(in super::super) process: Supervised,
  pub(in super::super) notifier: Notifier,
  pub(in super::super) events: mpsc::UnboundedSender<WorkerEvent>,
  pub(in super::super) stops: mpsc::UnboundedReceiver<Stop>,
}

struct Watch {
  db: ProjectDb,
  invocation: Invocation,
  tracker: InvocationTracker,
  pending: Vec<(Timestamp, AgentEvent)>,
  stop: Option<StopCause>,
  /// The budget a turn used up, ending the agent at its next turn or after the grace.
  over: Option<StopCause>,
}

impl Worker {
  /// Follows the CLI until it exits, then tells the run how it ended.
  pub(in super::super) async fn work(self) {
    let execution = self.invocation.execution;
    let events = self.events.clone();
    let end = self.follow().await;
    // The run's task outlives its agents; if it is gone, the runner is ending anyway.
    if events.send(WorkerEvent::Ended { execution, end }).is_err() {
      tracing::debug!(%execution, "the run no longer waits for its agent");
    }
  }

  async fn follow(mut self) -> Result<Finished, StoreError> {
    let mut watch = Watch {
      db: ProjectDb::open(&self.root)?,
      invocation: self.invocation.clone(),
      tracker: InvocationTracker::default(),
      pending: Vec::new(),
      stop: None,
      over: None,
    };
    let mut flush = tokio::time::interval(FLUSH_INTERVAL);
    let grace = sleep(Duration::ZERO);
    tokio::pin!(grace);
    let mut grace_armed = false;
    let mut listening = true;
    loop {
      tokio::select! {
        event = self.process.next() => match event {
          Some(ProcessEvent::Output { stream, line }) => {
            self.observe(&mut watch, parse_codex_line(stream, &line))?;
            if watch.stop.is_some() {
              self.process.terminate();
            }
          }
          Some(ProcessEvent::Exited(exit)) => return self.finish(watch, &exit),
          None => return self.finish(watch, &ProcessExit::WaitFailed { reason: String::new() }),
        },
        _ = flush.tick() => self.flush(&mut watch)?,
        stop = self.stops.recv(), if listening => match stop {
          Some(Stop::Now(cause)) => {
            if watch.stop.is_none() {
              watch.stop = Some(cause);
              self.process.terminate();
            }
          }
          Some(Stop::AfterTurn(cause)) => {
            if watch.over.is_none() && watch.stop.is_none() {
              watch.over = Some(cause);
              grace.as_mut().reset(Instant::now() + TURN_GRACE);
              grace_armed = true;
            }
          }
          None => listening = false,
        },
        () = &mut grace, if grace_armed && watch.stop.is_none() => {
          watch.stop = watch.over;
          self.process.terminate();
        }
      }
    }
  }

  /// Takes in one event: a turn's usage goes to the run, and a new turn after a used-up budget
  /// ends the agent.
  fn observe(&self, watch: &mut Watch, event: AgentEvent) -> Result<(), StoreError> {
    watch.tracker.observe(&event);
    let usage = match &event {
      AgentEvent::TurnCompleted { usage } => Some(*usage),
      _ => None,
    };
    if event == AgentEvent::TurnStarted && watch.stop.is_none() {
      watch.stop = watch.over;
    }
    watch.pending.push((Timestamp::now(), event));
    if watch.pending.len() >= FLUSH_BATCH || usage.is_some() {
      self.flush(watch)?;
    }
    if let Some(usage) = usage {
      let usage = WorkerEvent::Usage {
        execution: watch.invocation.execution,
        tokens: usage.counted_tokens(),
      };
      if self.events.send(usage).is_err() {
        tracing::debug!("the run no longer counts its agent's usage");
      }
    }
    Ok(())
  }

  fn flush(&self, watch: &mut Watch) -> Result<(), StoreError> {
    if watch.pending.is_empty() {
      return Ok(());
    }
    // A resumed session may not report its id again; the invocation keeps the one it resumed.
    let session = watch
      .tracker
      .session()
      .map(str::to_owned)
      .or_else(|| watch.invocation.session.clone());
    if watch.invocation.session != session || watch.invocation.usage != watch.tracker.usage() {
      watch.invocation.session = session;
      watch.invocation.usage = watch.tracker.usage();
      watch.db.save_invocation(&watch.invocation)?;
    }
    let events = std::mem::take(&mut watch.pending);
    let sequence = watch.db.append_activity(&watch.invocation, &events)?;
    self.notifier.send(Event::AgentActivity {
      run: watch.invocation.run,
      sequence,
    });
    Ok(())
  }

  fn finish(&self, mut watch: Watch, exit: &ProcessExit) -> Result<Finished, StoreError> {
    self.flush(&mut watch)?;
    let outcome = match (exit, watch.stop.is_some()) {
      (ProcessExit::Terminated, _) | (_, true) => InvocationOutcome::Terminated,
      (ProcessExit::Exited { code }, false) => watch.tracker.outcome(*code),
      (ProcessExit::WaitFailed { .. }, false) => InvocationOutcome::ExitFailed { code: None },
    };
    watch.invocation.ended_at = Some(Timestamp::now());
    watch.invocation.outcome = Some(outcome.clone());
    watch.db.save_invocation(&watch.invocation)?;
    Ok(Finished {
      outcome,
      stopped: watch.stop,
      final_message: watch.tracker.last_message().map(str::to_owned),
    })
  }
}
