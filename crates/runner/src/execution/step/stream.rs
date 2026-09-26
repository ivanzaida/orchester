//! Following an agent CLI while it works: its output is recorded as it arrives, the usage of each
//! turn is added to the run, and the agent is ended when the run reaches a limit or a request
//! (cancel, the runner stopping) needs it gone. A pause does not end the agent: it is recorded and
//! waits for the invocation to end.

use std::time::Duration;

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome, InvocationTracker, parse_codex_line},
  run::{Invocation, RunLimit},
  store::ActivityStore,
  time::Timestamp,
};
use tokio::time::{Instant, sleep};

use super::super::{
  idle,
  run::{ExecutionError, RunTask},
};
use crate::supervisor::{ProcessEvent, ProcessExit, Supervised};

/// Transcript entries are written at least this often while the CLI prints…
const FLUSH_INTERVAL: Duration = Duration::from_millis(250);
/// …or as soon as this many are waiting.
const FLUSH_BATCH: usize = 64;
/// How long an agent whose turn used up the budget may take to end on its own: usage arrives when
/// a turn completes, and the CLI usually exits right after. Starting another turn ends it at once.
const TURN_GRACE: Duration = Duration::from_secs(5);

/// Why the runner ended the agent.
pub(super) enum Stop {
  /// A cancel or the runner stopping; carried out once the agent has ended.
  Requested,
  Limit(RunLimit),
}

/// Follows one invocation.
struct Watch {
  tracker: InvocationTracker,
  pending: Vec<(Timestamp, AgentEvent)>,
  stop: Option<Stop>,
  /// The limit a turn used up: the run's budget or the project's daily budget.
  over: Option<RunLimit>,
}

impl RunTask {
  /// Follows the CLI until it exits, recording its output as it arrives, for at most `time_left`
  /// of active time.
  pub(super) async fn stream(
    &mut self,
    invocation: &mut Invocation,
    mut process: Supervised,
    time_left: Duration,
  ) -> Result<(InvocationOutcome, Option<Stop>), ExecutionError> {
    let mut watch = Watch {
      tracker: InvocationTracker::default(),
      pending: Vec::new(),
      stop: None,
      over: None,
    };
    let mut flush = tokio::time::interval(FLUSH_INTERVAL);
    let limit = sleep(time_left);
    tokio::pin!(limit);
    let mut limit_reason = RunLimit::Time;
    let mut watching = true;
    loop {
      tokio::select! {
        event = process.next() => match event {
          Some(ProcessEvent::Output { stream, line }) => {
            let event = parse_codex_line(stream, &line);
            if let Some(reached) = self.observe(invocation, &mut watch, event)?
              && limit.deadline() > Instant::now() + TURN_GRACE
            {
              limit.as_mut().reset(Instant::now() + TURN_GRACE);
              limit_reason = reached;
            }
            if watch.stop.is_some() {
              process.terminate();
            }
          }
          Some(ProcessEvent::Exited(exit)) => {
            self.flush(invocation, &mut watch)?;
            let outcome = outcome(&watch.tracker, &exit, watch.stop.is_some());
            return Ok((outcome, watch.stop));
          }
          None => {
            self.flush(invocation, &mut watch)?;
            return Ok((InvocationOutcome::ExitFailed { code: None }, watch.stop));
          }
        },
        _ = flush.tick() => self.flush(invocation, &mut watch)?,
        control = self.controls.recv(), if watching => match control {
          Some(control) => {
            if self.control(control)? && watch.stop.is_none() {
              watch.stop = Some(Stop::Requested);
              process.terminate();
            }
          }
          None => watching = false,
        },
        () = &mut limit, if watch.stop.is_none() => {
          watch.stop = Some(Stop::Limit(limit_reason));
          process.terminate();
        }
      }
    }
  }

  /// Takes in one event. Usage is added to the run as each turn completes; a turn that uses up
  /// the run's budget or the project's daily budget starts the grace for the CLI to end (the
  /// limit it reached), and a new turn after it ends the agent.
  fn observe(
    &mut self,
    invocation: &mut Invocation,
    watch: &mut Watch,
    event: AgentEvent,
  ) -> Result<Option<RunLimit>, ExecutionError> {
    watch.tracker.observe(&event);
    let usage = match &event {
      AgentEvent::TurnCompleted { usage } => Some(*usage),
      _ => None,
    };
    let new_turn = event == AgentEvent::TurnStarted;
    watch.pending.push((Timestamp::now(), event));
    if watch.pending.len() >= FLUSH_BATCH || usage.is_some() {
      self.flush(invocation, watch)?;
    }
    if new_turn
      && let Some(limit) = watch.over
      && watch.stop.is_none()
    {
      watch.stop = Some(Stop::Limit(limit));
    }
    let Some(usage) = usage else {
      return Ok(None);
    };
    self.transition(|run, now| run.record_usage(usage.counted_tokens(), now))?;
    if watch.over.is_some() {
      return Ok(None);
    }
    watch.over = if self.run.token_limit_reached() {
      Some(RunLimit::Tokens)
    } else if idle::daily_budget_used(&self.db)? {
      Some(RunLimit::DailyBudget)
    } else {
      None
    };
    Ok(watch.over)
  }

  fn flush(&mut self, invocation: &mut Invocation, watch: &mut Watch) -> Result<(), ExecutionError> {
    if watch.pending.is_empty() {
      return Ok(());
    }
    // A resumed session may not report its id again; the invocation keeps the one it resumed.
    let session = watch
      .tracker
      .session()
      .map(str::to_owned)
      .or_else(|| invocation.session.clone());
    if invocation.session != session || invocation.usage != watch.tracker.usage() {
      invocation.session = session;
      invocation.usage = watch.tracker.usage();
      self.db.save_invocation(invocation)?;
    }
    let events = std::mem::take(&mut watch.pending);
    self.record_activity(invocation, &events)
  }
}

fn outcome(tracker: &InvocationTracker, exit: &ProcessExit, stopped: bool) -> InvocationOutcome {
  match (exit, stopped) {
    (ProcessExit::Terminated, _) | (_, true) => InvocationOutcome::Terminated,
    (ProcessExit::Exited { code }, false) => tracker.outcome(*code),
    (ProcessExit::WaitFailed { .. }, false) => InvocationOutcome::ExitFailed { code: None },
  }
}
