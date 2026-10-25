//! Following one agent CLI while it works, beside the other agents of its run: its output is
//! recorded as it arrives, the usage of each turn is handed to the run, which decides about limits
//! and budgets, and the agent is ended when the run says so. A run that stops at a token limit or
//! a step budget lets the turn that used it end: the agent has [`TURN_GRACE`] to exit on its own,
//! and starting another turn ends it at once. What the agent used before its first file edit is
//! followed from the same events and stored with the invocation ([`FirstEditTracker`]). A person's
//! messages sent now reach the agent while its turn runs ([`steer`]). A native agent's loop runs in a
//! task of its own and reports to the worker as a CLI's output would ([`native`]).

mod native;
mod observe;
mod process;
mod review;
mod steer;

use std::{
  collections::HashSet,
  convert::Infallible,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  code_index::CodeIndexUse,
  command::SecretVariable,
  connection::stream::{AgentEvent, ClaudeStream, InvocationOutcome, InvocationTracker},
  conversation::Message,
  id::{AgentAccountId, ResourceId, StepExecutionId, TaskId},
  native::session::StopMode,
  orchester_tools::ProjectTool,
  permission::category::CommandScope,
  question::QuestionRequest,
  run::{Answerer, ApprovalAnswer, FirstEditTracker, Invocation, PendingToolApproval, RunLimit},
  store::ActivityStore,
  time::Timestamp,
  tokens::TokensUsed,
};
use orchester_store::{ProjectDb, StoreError};
use tokio::sync::mpsc;

pub(in crate::execution) use self::review::{ReviewerKeys, estimate_task};
use super::{
  super::{Notifier, control::Ack, tool::CommandFinished},
  app_server::AppServerSession,
  approval::Awaiting,
  claude_approval::ClaudeApprovals,
  native::NativeHandle,
  question::{AwaitingAnswer, QuestionAnswer},
  relay::Listening,
};
use crate::{private_file::PrivateFile, supervisor::Supervised};

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
  /// A pause checkpoints a session waiting for an unexecuted tool decision.
  Paused,
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
  /// Let ordinary work finish, but checkpoint instead of waiting for a tool decision.
  AtApproval,
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
  pub answer: ApprovalAnswer,
  /// Who gives the answer: the person in the app, or automation.
  pub by: Answerer,
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
  /// The turn failed only because the permission prompt or tool relay closed after the CLI had
  /// delivered its result (run 26): a question in that result still asks the person.
  pub(in super::super) closed_after_result: bool,
}

/// How the runner follows a CLI: it reads Claude Code's `stream-json` output, and talks with Codex
/// App Server.
pub(super) enum Transport {
  Claude(Box<ClaudeStream>),
  AppServer(Box<AppServerSession>),
}

/// What does the agent's work (`docs/architecture/native-agent.md` §2.2): a CLI's process, followed
/// through its transport, or the task that runs a native agent's loop.
pub(super) enum Driver {
  Process { transport: Transport, process: Supervised },
  Native(NativeHandle),
}

impl Driver {
  /// The CLI's process; a native agent has none.
  pub(super) const fn process(&self) -> Option<&Supervised> {
    match self {
      Self::Process { process, .. } => Some(process),
      Self::Native(_) => None,
    }
  }

  /// Ends the agent now: the CLI's process tree, or the native loop with what it runs.
  pub(super) fn end(&mut self) {
    match self {
      Self::Process { process, .. } => process.terminate(),
      Self::Native(handle) => handle.stop(StopMode::Now),
    }
  }
}

/// One agent at work and where its news goes.
pub(in super::super) struct Worker {
  pub(in super::super) root: PathBuf,
  pub(in super::super) invocation: Invocation,
  pub(super) driver: Driver,
  pub(in super::super) notifier: Notifier,
  pub(in super::super) events: mpsc::UnboundedSender<WorkerEvent>,
  pub(in super::super) stops: mpsc::UnboundedReceiver<Stop>,
  /// The MCP configuration the CLI read, removed once the CLI has ended.
  pub(in super::super) mcp_file: Option<PrivateFile>,
  /// The runner's end of Claude Code's permission prompt, when the step asks a person.
  pub(super) claude_approvals: Option<ClaudeApprovals>,
  /// The runner's end of Codex's relay of Orchester's tools, which asks about their calls.
  pub(super) tool_relay: Option<Listening>,
  /// Secret values kept out of the invocation's activity and approvals: those of a Claude Code
  /// invocation's MCP servers, a native agent's API key.
  pub(in super::super) secrets: Vec<SecretVariable>,
  pub(in super::super) agent: String,
  /// The account the agent's CLI runs on, whose plan limits it reports; `None` for a native agent.
  pub(in super::super) account: Option<AgentAccountId>,
  pub(in super::super) task: TaskId,
  pub(in super::super) decisions: mpsc::UnboundedReceiver<ApprovalDecision>,
  /// Answers to the question the agent waits on in its turn.
  pub(in super::super) answers: mpsc::UnboundedReceiver<QuestionAnswer>,
  /// Where the agent's commands run and may write, which classifies the commands it asks about.
  pub(in super::super) scope: CommandScope,
  /// The agent profile of the step, which Always allow of a command adds to.
  pub(in super::super) profile: Option<ResourceId>,
  /// The checks the step is offered, whose access rules decide whether a `run_check` call may be approved similar
  /// and whether one approval covers the run.
  pub(in super::super) checks: Vec<ProjectTool>,
  /// People's messages sent now, for the running turn.
  pub(in super::super) steers: mpsc::UnboundedReceiver<Vec<Message>>,
  /// Auto mode's review of the waiting approval, while it is under way.
  pub(super) reviewing: Option<review::Reviewing>,
  /// Where a native reviewer's API key is read.
  pub(in crate::execution::step) reviewer_keys: review::ReviewerKeys,
  /// Held by each review until it has recorded what it cost; [`Worker::work`] waits for all of them.
  pub(in crate::execution::step) reviews_held: mpsc::Sender<Infallible>,
  /// The run's QA apps: the invocation's stops while it waits for a person.
  pub(in crate::execution::step) qa: std::sync::Arc<super::QaApps>,
}

/// Which call the relay held when it closed.
#[derive(Clone, Copy)]
enum RelayClosed {
  Decision,
  Question,
}

pub(super) struct Watch {
  pub(super) db: ProjectDb,
  pub(super) invocation: Invocation,
  pub(super) tracker: InvocationTracker,
  edits: FirstEditTracker,
  /// The edit progress changed since it was stored.
  edits_changed: bool,
  pending: Vec<(Timestamp, AgentEvent)>,
  pub(super) stop: Option<StopCause>,
  /// The budget a turn used up, ending the agent at its next turn or after the grace.
  over: Option<StopCause>,
  /// Commands started and not finished yet.
  commands: HashSet<String>,
  /// Ends the agent once no command runs.
  after_command: Option<StopCause>,
  pause_requested: bool,
  /// The approval waiting for a person, and the call it answers.
  pub(super) approval: Option<(PendingToolApproval, Awaiting)>,
  /// The question the agent waits on in its turn, and the request or call its answers go to.
  pub(super) question: Option<(QuestionRequest, AwaitingAnswer)>,

  /// Claude Code calls a person approved whose results have not arrived: if the agent ends before
  /// they do, the actions may have run, and the run is not resumed on its own.
  pub(super) approved_calls: HashSet<String>,
  /// Messages sent now that the CLI has not taken yet.
  steering: steer::Steering,
  /// The turn failure recorded when the prompt or relay closed after the CLI delivered its result.
  pub(super) closed_after_result: Option<String>,
  /// A completed Codex turn left a call of Orchester's tools outstanding.
  tools_after_turn: bool,
  /// The relay's returns ([`Listening::returns`]) when Codex was last continued: a further continuation needs one
  /// since.
  returns_continued: u64,
  /// Continuations since a decision was settled or a call returned that Codex waited for after its turn
  /// (`process::MAX_IDLE_CONTINUATIONS`).
  pub(super) idle_continuations: u8,
  /// The agent's account is at its plan limit, as its CLI's last plan reading said: a turn that fails now stops at
  /// that limit instead of failing the step.
  pub(super) plan_reached: bool,
}

impl Watch {
  /// Follows `invocation`, whose searches of the code index count when it got the index, as recorded before it started.
  fn open(root: &Path, invocation: &Invocation) -> Result<Self, StoreError> {
    let db = ProjectDb::open(root)?;
    let index_given = matches!(
      db.invocation_code_index(invocation.id)?,
      Some(CodeIndexUse::Given { .. })
    );
    Ok(Self {
      db,
      invocation: invocation.clone(),
      tracker: InvocationTracker::default(),
      edits: FirstEditTracker::new(index_given),
      edits_changed: false,
      pending: Vec::new(),
      stop: None,
      over: None,
      commands: HashSet::new(),
      after_command: None,
      pause_requested: false,
      approval: None,
      question: None,
      approved_calls: HashSet::new(),
      steering: steer::Steering::default(),
      closed_after_result: None,
      tools_after_turn: false,
      returns_continued: 0,
      idle_continuations: 0,
      plan_reached: false,
    })
  }

  /// Ready once a call the relay holds for a person can no longer be answered: a Claude Code call waiting
  /// for a decision, or an `ask_question` call waiting for the answer.
  async fn relay_closed(&mut self) -> RelayClosed {
    let approval = async {
      match &mut self.approval {
        Some((_, Awaiting::Claude { reply, .. } | Awaiting::Tool { reply, .. })) => reply.closed().await,
        _ => std::future::pending().await,
      }
    };
    let question = async {
      match &mut self.question {
        Some((_, AwaitingAnswer::Relay(reply))) => reply.closed().await,
        _ => std::future::pending().await,
      }
    };
    tokio::select! {
      () = approval => RelayClosed::Decision,
      () = question => RelayClosed::Question,
    }
  }
}

impl Worker {
  /// The App Server conversation, for Codex.
  pub(super) fn app_server_mut(&mut self) -> Option<&mut AppServerSession> {
    match &mut self.driver {
      Driver::Process {
        transport: Transport::AppServer(app),
        ..
      } => Some(&mut **app),
      Driver::Process { .. } | Driver::Native(_) => None,
    }
  }

  pub(super) const fn app_server(&self) -> Option<&AppServerSession> {
    match &self.driver {
      Driver::Process {
        transport: Transport::AppServer(app),
        ..
      } => Some(&**app),
      Driver::Process { .. } | Driver::Native(_) => None,
    }
  }

  /// Whether a native agent does the work.
  pub(super) const fn is_native(&self) -> bool {
    matches!(self.driver, Driver::Native(_))
  }

  /// Follows the agent until it ends, then tells the run how it ended, once the reviews it started have
  /// ended: the agent's end stops a review still under way, which then records what it cost and hands its
  /// tokens to the run before the run hears of the end. So the run, still listening, counts them toward
  /// its budget and the step's, once, before another invocation of the step reads what the step used.
  /// `reviews_ended` is the other end of [`Worker::reviews_held`].
  pub(in super::super) async fn work(mut self, mut reviews_ended: mpsc::Receiver<Infallible>) {
    let execution = self.invocation.execution;
    let events = self.events.clone();
    let mcp_file = self.mcp_file.take();
    let end = if self.is_native() {
      self.follow_native().await
    } else {
      self.follow().await
    };
    // The worker, its own holder and the review under way are gone; this ends once every review has.
    if let Some(never) = reviews_ended.recv().await {
      match never {}
    }
    // The CLI has ended: the configuration it read, with its secret values, goes before the run hears of it.
    drop(mcp_file);
    // The run's task outlives its agents; if it is gone, the runner is ending anyway.
    if events.send(WorkerEvent::Ended { execution, end }).is_err() {
      tracing::debug!(%execution, "the run no longer waits for its agent");
    }
  }
}
