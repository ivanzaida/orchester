//! The native agent loop as a state machine without network or processes
//! (`docs/architecture/native-agent.md` §2.1): given a model response it plans the tool calls, given a
//! call's outcome it decides what comes next. The driver in the runner does the work it is told to
//! ([`LoopAction`]) and reports back ([`LoopInput`]).
//!
//! A turn's tool calls run in order, the reading ones first and together (§4.10). A call that cannot run
//! (invalid input, a refused or denied command) is answered with an error result and the turn goes on
//! (decision 2). `finish_step` ends the loop with the step's result and `ask_question` with a question;
//! calls after them are not run. Before every model request the stop modes are checked and, when the
//! conversation nears the model's context window, it is compacted (§6.3).

mod context;
mod events;
mod protocol;
mod round;

use std::collections::{BTreeSet, VecDeque};

pub use self::{
  context::COMPACTION_REQUEST,
  protocol::{
    ApprovalAsk, Continuation, DEFAULT_CONTEXT_WINDOW, LoopAction, LoopEnd, LoopError, LoopFailure, LoopInput,
    LoopSettings, MAX_COMPACTIONS, PlannedTool, StopMode, ToolOutcome, ToolWork,
  },
  round::{DENIED, RECORDED, REMINDER},
};
use self::{context::ContextEstimate, round::Queued};
use super::{
  command::{CommandPolicy, ParsedCommand},
  item::{Item, ToolCallId, UserOrigin},
  model::ModelRequest,
  scope::FileScope,
};
use crate::connection::stream::AgentEvent;

/// The result a call gets that was left unanswered when an invocation stopped (§2.2).
pub const NOT_RUN: &str = "Not run: the step was stopped before this call ran. Call it again if it is still needed.";

/// Where the loop is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
  AwaitingModel,
  /// Waiting for the summary of a compaction request.
  Compacting,
  /// These calls run, each with its Activity label.
  Running(Vec<(ToolCallId, String)>),
  AwaitingDecision(ToolCallId, ParsedCommand),
  Ended,
}

impl Phase {
  const fn name(&self) -> &'static str {
    match self {
      Self::AwaitingModel => "waits for the model",
      Self::Compacting => "waits for a summary",
      Self::Running(_) => "runs tool calls",
      Self::AwaitingDecision(..) => "waits for a decision",
      Self::Ended => "has ended",
    }
  }
}

/// One invocation's loop over a native conversation.
#[derive(Debug)]
pub struct NativeLoop {
  settings: LoopSettings,
  scope: FileScope,
  commands: CommandPolicy,
  /// The first item, the step's prompt, which every segment starts with.
  task: Item,
  /// The items of the current segment, as the next request sends them.
  items: Vec<Item>,
  phase: Phase,
  queue: VecDeque<Queued>,
  stop: Option<StopMode>,
  /// The model was reminded to call `finish_step` after its last turn, which had no tool call.
  reminded: bool,
  compactions: u8,
  context: ContextEstimate,
  /// Files the agent changed in this step, earlier invocations included, named after a compaction.
  changed: BTreeSet<String>,
}

impl NativeLoop {
  /// A new conversation that starts with the step's prompt.
  pub fn start(
    settings: LoopSettings,
    scope: FileScope,
    commands: CommandPolicy,
    prompt: String,
  ) -> (Self, Vec<LoopAction>) {
    let task = Item::user(prompt, UserOrigin::Task);
    let mut actions = vec![
      LoopAction::Persist(vec![task.clone()]),
      LoopAction::Emit(vec![events::session(&settings)]),
    ];
    let mut native = Self::new(settings, scope, commands, task.clone(), vec![task]);
    native.next_request(&mut actions);
    (native, actions)
  }

  /// The stored conversation `stored` continued: calls left unanswered get [`NOT_RUN`] (a pending
  /// `ask_question` the person's answer), then `continuation` follows. `changed` holds the files the
  /// step's earlier invocations changed, as their `FileChanged` Activity recorded them; a compaction
  /// names them with the files this invocation changes.
  pub fn resume(
    settings: LoopSettings,
    scope: FileScope,
    commands: CommandPolicy,
    stored: Vec<Item>,
    continuation: Continuation,
    changed: BTreeSet<String>,
  ) -> Result<(Self, Vec<LoopAction>), LoopError> {
    let (task, mut items) = context::latest_segment(stored)?;
    let appended = context::answer_unanswered(&items, continuation);
    items.extend(appended.iter().cloned());
    let mut actions = vec![
      LoopAction::Persist(appended),
      LoopAction::Emit(vec![events::session(&settings)]),
    ];
    let mut native = Self::new(settings, scope, commands, task, items);
    native.changed = changed;
    native.next_request(&mut actions);
    Ok((native, actions))
  }

  fn new(settings: LoopSettings, scope: FileScope, commands: CommandPolicy, task: Item, items: Vec<Item>) -> Self {
    let context = ContextEstimate::fresh(&settings, &items);
    Self {
      settings,
      scope,
      commands,
      task,
      items,
      phase: Phase::AwaitingModel,
      queue: VecDeque::new(),
      stop: None,
      reminded: false,
      compactions: 0,
      context,
      changed: BTreeSet::new(),
    }
  }

  /// What to do after `input`.
  pub fn handle(&mut self, input: LoopInput) -> Result<Vec<LoopAction>, LoopError> {
    let mut actions = Vec::new();
    match input {
      LoopInput::Responded(response) => match self.phase {
        Phase::AwaitingModel => self.on_response(response, &mut actions),
        Phase::Compacting => self.on_summary(&response, &mut actions),
        _ => return Err(self.unexpected("a model response")),
      },
      LoopInput::ModelFailed(error) => self.on_model_failed(error, &mut actions)?,
      LoopInput::ToolDone(call, outcome) => self.on_tool_done(call, outcome, &mut actions)?,
      LoopInput::Decided(call, approved) => self.on_decided(call, approved, &mut actions)?,
      LoopInput::Stop(mode) => self.on_stop(mode, &mut actions),
      LoopInput::Withdraw => {
        if matches!(self.stop, Some(StopMode::AfterCommand | StopMode::AtApproval)) {
          self.stop = None;
        }
      }
    }
    Ok(actions)
  }

  /// Whether the loop has ended; it then expects no more input.
  pub fn has_ended(&self) -> bool {
    self.phase == Phase::Ended
  }

  fn on_stop(&mut self, mode: StopMode, actions: &mut Vec<LoopAction>) {
    if self.phase == Phase::Ended {
      return;
    }
    let ends_now = match mode {
      StopMode::Now => true,
      StopMode::AfterTurn => matches!(self.phase, Phase::AwaitingDecision(..)),
      StopMode::AfterCommand => !matches!(self.phase, Phase::Running(_)),
      StopMode::AtApproval => matches!(self.phase, Phase::AwaitingDecision(..)),
    };
    if ends_now {
      self.end(LoopEnd::Stopped(mode), actions);
    } else if self.stop.is_none_or(|current| strength(mode) > strength(current)) {
      self.stop = Some(mode);
    }
  }

  fn on_model_failed(
    &mut self,
    error: super::model::ModelError,
    actions: &mut Vec<LoopAction>,
  ) -> Result<(), LoopError> {
    use super::model::ModelError;
    match (&self.phase, error) {
      (Phase::AwaitingModel, ModelError::TooLarge) => {
        self.context.force();
        self.next_request(actions);
      }
      (Phase::Compacting, ModelError::TooLarge) => self.end(LoopEnd::Failed(LoopFailure::ContextExhausted), actions),
      (Phase::AwaitingModel | Phase::Compacting, error) => {
        self.end(LoopEnd::Failed(LoopFailure::Model(error)), actions)
      }
      _ => return Err(self.unexpected("a model failure")),
    }
    Ok(())
  }

  /// The next model request, unless a stop or the context window ends the loop first.
  fn next_request(&mut self, actions: &mut Vec<LoopAction>) {
    if let Some(mode @ (StopMode::AfterTurn | StopMode::AfterCommand)) = self.stop {
      self.end(LoopEnd::Stopped(mode), actions);
      return;
    }
    if self.context.needs_compaction(&self.settings) {
      self.compact(actions);
      return;
    }
    self.phase = Phase::AwaitingModel;
    actions.push(LoopAction::Emit(vec![AgentEvent::TurnStarted]));
    actions.push(LoopAction::CallModel(self.request(Vec::new())));
  }

  /// A request with the current segment, followed by `extra` items that are not part of it.
  fn request(&self, extra: Vec<Item>) -> ModelRequest {
    let mut items = self.items.clone();
    items.extend(extra);
    ModelRequest {
      model: self.settings.model.clone(),
      effort: self.settings.effort.clone(),
      max_output: self.settings.max_output,
      system: self.settings.system.clone(),
      tools: self.settings.tools.clone(),
      items,
      cache_key: self.settings.conversation,
    }
  }

  /// Appends `items` to the segment and has them stored.
  fn append(&mut self, items: Vec<Item>, actions: &mut Vec<LoopAction>) {
    self.context.appended(&items);
    self.items.extend(items.iter().cloned());
    actions.push(LoopAction::Persist(items));
  }

  fn end(&mut self, end: LoopEnd, actions: &mut Vec<LoopAction>) {
    self.phase = Phase::Ended;
    self.queue.clear();
    actions.push(LoopAction::End(end));
  }

  fn unexpected(&self, input: &'static str) -> LoopError {
    LoopError::Unexpected {
      input,
      state: self.phase.name(),
    }
  }
}

/// Which of two waiting stops wins: the one that ends sooner.
const fn strength(mode: StopMode) -> u8 {
  match mode {
    StopMode::AtApproval => 0,
    StopMode::AfterCommand => 1,
    StopMode::AfterTurn => 2,
    StopMode::Now => 3,
  }
}
