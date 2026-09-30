//! Following one agent CLI while it works, beside the other agents of its run: its output is
//! recorded as it arrives, the usage of each turn is handed to the run, which decides about limits
//! and budgets, and the agent is ended when the run says so. A run that stops at a token limit or
//! a step budget lets the turn that used it end: the agent has [`TURN_GRACE`] to exit on its own,
//! and starting another turn ends it at once.

use std::{
  collections::HashSet,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  command::SecretVariable,
  connection::{
    app_server::Update,
    stream::{AgentEvent, AgentStream, InvocationOutcome, InvocationTracker},
  },
  id::{StepExecutionId, TaskId},
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

use super::{
  super::{Notifier, control::Ack, tool::CommandFinished},
  app_server::{AppServerFailure, AppServerSession},
  approval::Awaiting,
  claude_approval::ClaudeApprovals,
};
use crate::{
  private_file::PrivateFile,
  supervisor::{ProcessEvent, ProcessExit, Supervised},
};

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
  /// A limit of the run, or the budget of this agent's step.
  Limit(RunLimit),
  /// Another step failed and its attempt ended; this execution was skipped.
  Abandoned,
  /// The run's permissions changed with Apply now: the agent restarts with them.
  Permissions,
}

/// What the run tells an agent.
#[derive(Clone, Copy, Debug)]
pub(in super::super) enum Stop {
  /// End it now.
  Now(StopCause),
  /// Its turn used up a budget: end it once the turn is over.
  AfterTurn(StopCause),
  /// End it once the command it is running has finished, or now when it runs none.
  AfterCommand(StopCause),
  /// Forget an [`Stop::AfterCommand`] that has not happened yet.
  Withdraw,
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
  /// A command tool's program ended; `Err` when recording what it did failed.
  CommandEnded {
    execution: StepExecutionId,
    end: Result<CommandFinished, StoreError>,
  },
}

pub(in super::super) struct ApprovalDecision {
  pub id: String,
  pub approve: bool,
  pub ack: Ack,
}

/// How an invocation ended.
pub(in super::super) struct Finished {
  pub(in super::super) outcome: InvocationOutcome,
  /// Why the run ended the agent; `None` when it exited on its own.
  pub(in super::super) stopped: Option<StopCause>,
  /// The last message the agent wrote.
  pub(in super::super) final_message: Option<String>,
  /// A pending or delivered decision ended without a confirmed turn completion.
  pub(in super::super) approval_interrupted: bool,
}

/// One running agent CLI and where its news goes.
pub(in super::super) struct Worker {
  pub(in super::super) root: PathBuf,
  pub(in super::super) invocation: Invocation,
  /// Reads the CLI's output with the parser of its provider.
  pub(in super::super) output: AgentStream,
  pub(in super::super) process: Supervised,
  pub(in super::super) notifier: Notifier,
  pub(in super::super) events: mpsc::UnboundedSender<WorkerEvent>,
  pub(in super::super) stops: mpsc::UnboundedReceiver<Stop>,
  /// The MCP configuration the CLI read, removed once the CLI has ended.
  pub(in super::super) mcp_file: Option<PrivateFile>,
  pub(super) app_server: Option<AppServerSession>,
  /// The runner's end of Claude Code's permission prompt, when the step asks a person.
  pub(super) claude_approvals: Option<ClaudeApprovals>,
  /// Secret values of a Claude Code invocation's MCP servers, kept out of its activity and approvals.
  pub(super) mcp_secrets: Vec<SecretVariable>,
  pub(in super::super) agent: String,
  pub(in super::super) task: TaskId,
  pub(in super::super) decisions: mpsc::UnboundedReceiver<ApprovalDecision>,
}

pub(super) struct Watch {
  pub(super) db: ProjectDb,
  pub(super) invocation: Invocation,
  pub(super) tracker: InvocationTracker,
  pending: Vec<(Timestamp, AgentEvent)>,
  pub(super) stop: Option<StopCause>,
  /// The budget a turn used up, ending the agent at its next turn or after the grace.
  over: Option<StopCause>,
  /// Commands started and not finished yet.
  commands: usize,
  /// Ends the agent once no command runs.
  after_command: Option<StopCause>,
  /// The approval waiting for a person, by its id.
  pub(super) approval: Option<(String, Awaiting)>,
  /// Claude Code calls a person approved whose results have not arrived: if the agent ends before
  /// they do, the actions may have run, and the run is not resumed on its own.
  pub(super) approved_calls: HashSet<String>,
}

impl Watch {
  /// Ready once the Claude Code call waiting for a person can no longer be answered.
  async fn prompt_closed(&mut self) {
    match &mut self.approval {
      Some((_, Awaiting::Claude { reply, .. })) => reply.closed().await,
      _ => std::future::pending().await,
    }
  }
}

impl Watch {
  fn open(root: &Path, invocation: &Invocation) -> Result<Self, StoreError> {
    Ok(Self {
      db: ProjectDb::open(root)?,
      invocation: invocation.clone(),
      tracker: InvocationTracker::default(),
      pending: Vec::new(),
      stop: None,
      over: None,
      commands: 0,
      after_command: None,
      approval: None,
      approved_calls: HashSet::new(),
    })
  }
}

impl Worker {
  /// Follows the CLI until it exits, then tells the run how it ended.
  pub(in super::super) async fn work(mut self) {
    let execution = self.invocation.execution;
    let events = self.events.clone();
    let mcp_file = self.mcp_file.take();
    let end = self.follow().await;
    // The CLI has ended: the configuration it read, with its secret values, goes before the run hears of it.
    drop(mcp_file);
    // The run's task outlives its agents; if it is gone, the runner is ending anyway.
    if events.send(WorkerEvent::Ended { execution, end }).is_err() {
      tracing::debug!(%execution, "the run no longer waits for its agent");
    }
  }

  async fn follow(mut self) -> Result<Finished, StoreError> {
    let mut watch = Watch::open(&self.root, &self.invocation)?;
    self.initialize_app_server(&mut watch).await?;
    let mut flush = tokio::time::interval(FLUSH_INTERVAL);
    let grace = sleep(Duration::ZERO);
    tokio::pin!(grace);
    let mut grace_armed = false;
    let mut listening = true;
    let mut deciding = self.app_server.is_some() || self.claude_approvals.is_some();
    loop {
      tokio::select! {
        event = self.process.next() => match event {
          Some(ProcessEvent::Output { stream, line }) => {
            if let Some(app) = self.app_server.as_mut() {
              match app.receive(stream, &line, &self.process).await {
                Ok(updates) => for update in updates { self.app_update(&mut watch, update)?; },
                Err(error) => {
                  let event = match error {
                    AppServerFailure::Protocol(message) => AgentEvent::TurnFailed { message },
                    AppServerFailure::NotSignedIn(message) => AgentEvent::NotSignedIn { message },
                  };
                  self.observe(&mut watch, event)?;
                  self.process.terminate();
                }
              }
            } else {
              for event in self.output.parse(stream, &line) {
                self.observe(&mut watch, event)?;
              }
            }
            if watch.after_command.is_some() && self.checkpoint_safe(&watch) && watch.stop.is_none() {
              watch.stop = watch.after_command.take();
            }
            if watch.stop.is_some() {
              self.process.terminate();
            }
          }
          Some(ProcessEvent::Exited(exit)) => return self.finish(watch, &exit),
          None => return self.finish(watch, &ProcessExit::WaitFailed { reason: String::new() }),
        },
        _ = flush.tick() => self.flush(&mut watch)?,
        decision = self.decisions.recv(), if deciding => match decision {
          Some(decision) => self.decide(&mut watch, decision).await?,
          None => deciding = false,
        },
        Some(asked) = ClaudeApprovals::next(self.claude_approvals.as_mut()) => self.request_claude_approval(&mut watch, asked)?,
        () = watch.prompt_closed() => self.prompt_closed(&mut watch)?,
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
          Some(Stop::AfterCommand(cause)) => {
            if watch.stop.is_none() && watch.over.is_none() {
              watch.after_command = Some(cause);
              if self.checkpoint_safe(&watch) || watch.approval.is_some() {
                watch.stop = watch.after_command.take();
                self.process.terminate();
              }
            }
          }
          Some(Stop::Withdraw) => watch.after_command = None,
          None => listening = false,
        },
        () = &mut grace, if grace_armed && watch.stop.is_none() => {
          watch.stop = watch.over;
          self.process.terminate();
        }
      }
    }
  }

  async fn initialize_app_server(&mut self, watch: &mut Watch) -> Result<(), StoreError> {
    if let Some(app) = &self.app_server
      && let Err(error) = app.start(&self.process).await
    {
      self.observe(
        watch,
        AgentEvent::TurnFailed {
          message: format!("cannot initialize Codex App Server: {error}"),
        },
      )?;
      self.process.terminate();
    }
    Ok(())
  }

  fn checkpoint_safe(&self, watch: &Watch) -> bool {
    watch.commands == 0 && !self.app_server.as_ref().is_some_and(AppServerSession::has_active_tools)
  }

  fn app_update(&mut self, watch: &mut Watch, update: Update) -> Result<(), StoreError> {
    match update {
      Update::Activity(event) => self.observe(watch, event),
      Update::Approval(request) => self.request_approval(watch, request),
      Update::TurnEnded => {
        self.process.terminate();
        Ok(())
      }
    }
  }

  /// Takes in one event: a turn's usage goes to the run, and a new turn after a used-up budget
  /// ends the agent.
  pub(super) fn observe(&self, watch: &mut Watch, mut event: AgentEvent) -> Result<(), StoreError> {
    if self.app_server.is_some() || !self.mcp_secrets.is_empty() {
      event.redact(&|text| self.redact(text));
    }
    watch.tracker.observe(&event);
    let usage = match &event {
      AgentEvent::TurnCompleted { usage } => Some(*usage),
      _ => None,
    };
    if event == AgentEvent::TurnStarted && watch.stop.is_none() {
      watch.stop = watch.over;
    }
    match &event {
      AgentEvent::CommandStarted { .. } => watch.commands += 1,
      AgentEvent::CommandFinished { item, .. } => {
        watch.commands = watch.commands.saturating_sub(1);
        watch.approved_calls.remove(item);
      }
      _ => {}
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
    let mut approval_interrupted = false;
    self.check_prompt_reached(&mut watch)?;
    if watch.approval.is_some()
      || !watch.approved_calls.is_empty()
      || self.app_server.as_ref().is_some_and(|app| !app.complete())
    {
      approval_interrupted = watch
        .db
        .interrupt_tool_approvals(watch.invocation.run, watch.invocation.id)?;
      watch.invocation.timing.resume(Timestamp::now());
      self.notifier.send(Event::ToolApprovalChanged {
        run: watch.invocation.run,
        task: self.task,
      });
    }
    self.flush(&mut watch)?;
    let reported = watch.tracker.outcome(Some(1));
    let outcome = if watch.stop.is_none()
      && matches!(
        reported,
        InvocationOutcome::TurnFailed { .. } | InvocationOutcome::NotSignedIn { .. }
      ) {
      reported
    } else {
      match (
        exit,
        watch.stop.is_some(),
        self.app_server.as_ref().is_some_and(AppServerSession::complete),
      ) {
        (_, false, true) => watch.tracker.outcome(Some(0)),
        (ProcessExit::Terminated, ..) | (_, true, _) => InvocationOutcome::Terminated,
        (ProcessExit::Exited { code }, false, _) => watch.tracker.outcome(*code),
        (ProcessExit::WaitFailed { .. }, false, _) => InvocationOutcome::ExitFailed { code: None },
      }
    };
    watch.invocation.ended_at = Some(Timestamp::now());
    watch.invocation.outcome = Some(outcome.clone());
    watch.db.save_invocation(&watch.invocation)?;
    Ok(Finished {
      outcome,
      stopped: watch.stop,
      final_message: watch.tracker.last_message().map(str::to_owned),
      approval_interrupted,
    })
  }
}
