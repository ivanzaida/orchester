//! What the driver of a [`super::NativeLoop`] feeds it and what it is told to do (§2.1).

use thiserror::Error;

use crate::{
  connection::stream::FileChange,
  id::ConversationId,
  native::{
    command::ParsedCommand,
    item::{Item, ToolCallId},
    model::{ModelError, ModelRequest, ModelResponse},
    tools::{AskQuestion, EditFile, ListFiles, ReadFile, Search, ToolSet, WriteFile},
  },
  resource::ModelId,
  run::StepResult,
};

/// The context window assumed when neither the endpoint nor the person named one (§6.3).
pub const DEFAULT_CONTEXT_WINDOW: u64 = 200_000;
/// Compactions one invocation may make before it ends with [`LoopFailure::ContextExhausted`].
pub const MAX_COMPACTIONS: u8 = 3;

/// What stays the same for the whole conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopSettings {
  pub conversation: ConversationId,
  pub model: ModelId,
  pub effort: Option<String>,
  pub max_output: u32,
  /// The model's context window in tokens; [`DEFAULT_CONTEXT_WINDOW`] when unknown.
  pub context_window: u64,
  pub system: String,
  pub tools: ToolSet,
}

/// How the run wants the invocation to stop, as the runner's stops for the CLIs (§2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StopMode {
  /// End at once; a running model request or command is dropped by the driver.
  Now,
  /// A budget is used up: let the current model response finish, run none of its tool calls, and end.
  AfterTurn,
  /// Let the running tool calls finish and record their results, then end; end at once when none runs.
  AfterCommand,
  /// Go on with ordinary work, but end instead of waiting for a person's decision.
  AtApproval,
}

/// What the driver tells the loop.
#[derive(Clone, Debug, PartialEq)]
pub enum LoopInput {
  Responded(ModelResponse),
  ModelFailed(ModelError),
  ToolDone(ToolCallId, ToolOutcome),
  /// A person approved (`true`) or denied the command of a call.
  Decided(ToolCallId, bool),
  Stop(StopMode),
  /// Forget a pause ([`StopMode::AfterCommand`], [`StopMode::AtApproval`]) that has not happened yet.
  Withdraw,
}

/// What the loop tells the driver to do, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum LoopAction {
  /// Store these items before doing what follows (§6.2).
  Persist(Vec<Item>),
  Emit(Vec<crate::connection::stream::AgentEvent>),
  CallModel(ModelRequest),
  /// Run these calls; several only when all of them read, and then together (§4.10).
  Run(Vec<PlannedTool>),
  /// Ask a person whether a command may run, and answer with [`LoopInput::Decided`].
  Ask(ApprovalAsk),
  End(LoopEnd),
}

/// How an invocation ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoopEnd {
  Result(StepResult),
  /// The agent asked the person; the answer resumes the conversation as this call's result.
  Question {
    call: ToolCallId,
    question: AskQuestion,
  },
  Stopped(StopMode),
  /// The model ended its turns without a result, also after a reminder.
  NoResult,
  Failed(LoopFailure),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LoopFailure {
  #[error(transparent)]
  Model(ModelError),
  #[error("the model declined to answer{}", .category.as_ref().map(|category| format!(" ({category})")).unwrap_or_default())]
  Refused { category: Option<String> },
  #[error("the provider's content filter cut the response")]
  Filtered,
  #[error("the conversation no longer fits the model's context window after {MAX_COMPACTIONS} compactions")]
  ContextExhausted,
  #[error("the model wrote no summary to compact the conversation with")]
  NoSummary,
}

/// A tool call to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedTool {
  pub call: ToolCallId,
  pub work: ToolWork,
}

/// What a planned call does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolWork {
  ReadFile(ReadFile),
  ListFiles(ListFiles),
  Search(Search),
  EditFile(EditFile),
  WriteFile(WriteFile),
  Command(ParsedCommand),
}

/// A command waiting for a person's decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalAsk {
  pub call: ToolCallId,
  pub command: ParsedCommand,
}

/// How a call ended, as the executor reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolOutcome {
  /// The result the model gets, already within the tool's output limits (§6.4).
  pub content: String,
  pub is_error: bool,
  /// What Activity shows: a command's output, a count for the file tools.
  pub activity: String,
  /// A command's exit code.
  pub exit_code: Option<i64>,
  /// Files the call added, changed or removed.
  pub changes: Vec<FileChange>,
}

/// What a resumed invocation starts with (§2.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Continuation {
  /// The person's answer: the result of the pending `ask_question` call, or a new user item when none
  /// is pending.
  Answer(String),
  /// The continue prompt, a revise note or messages, as a new user item.
  Prompt(String),
}

/// The driver fed the loop something its state does not expect; a defect of the driver.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LoopError {
  #[error("{input} is not expected while the loop {state}")]
  Unexpected { input: &'static str, state: &'static str },
  #[error("no running tool call has id {0}")]
  UnknownCall(ToolCallId),
  #[error("a stored conversation must start with the task's prompt")]
  NoTask,
}
