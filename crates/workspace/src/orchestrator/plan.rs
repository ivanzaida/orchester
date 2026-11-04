//! The Orchestrator's plan-only mode (ORC-646; `orchester_core::orchestrator::plan`, `docs/design/
//! side-question-and-plan-mode.md`, "Plan-only mode"). It is held where the chat's tools run, not only in what the
//! agent is told: in a turn that plans, every tool that changes something is refused (`plan_only`) before it reaches
//! the tool, and the refusal is kept in the conversation as any refused action is; the agent answers with a plan
//! through `propose_plan`, which works only in such a turn. A turn a wait, a run's end or a day's report started by
//! itself plans too while the conversation's switch is on. The person approves a plan, which keeps the message whose
//! ordinary turn carries it out and asks the runner for it; that turn acts by every rule a turn has, so what only a
//! person decides still comes as a card. *Change it* sends the person's correction as a turn that plans, whose plan
//! replaces the one that waited; *Dismiss* folds it.

use std::path::Path;

use orchester_core::{
  id::{ChatId, ChatMessageId},
  orchestrator::{ChatAction, ChatEntry, ChatPlan, EntryKind, PlanStep, StepKind, TurnMode, TurnState},
  run::Answerer,
  time::Timestamp,
};
use orchester_ipc::ChatTools;
use orchester_store::ProjectDb;
use rmcp::{
  handler::server::common::schema_for_type,
  model::{JsonObject, Tool, ToolAnnotations},
  schemars::JsonSchema,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  keep,
  send::{SendError, not_sent},
  tools::ChatScope,
};
use crate::{
  app_mcp::{ToolError, Translate, blocking},
  background,
  messages::{self, Message},
  runner::{RequestError, RunRequestError, Runners},
  workspace::OpenProject,
};

/// The tool's name.
pub const PROPOSE_PLAN: &str = "propose_plan";

/// What the chat's agent reads of the tool.
const DESCRIPTION: &str = "Answers a message sent in plan-only mode (it begins with [Plan only]) with a plan of the \
  actions you would take, for the person to approve, change or dismiss: its steps in order, each with its kind (run \
  for starting, continuing, pausing or merging a run; draft for creating or changing a task or a draft; setting for \
  changing a setting of the project; other for anything else, such as a message to an agent), what will happen and \
  what it acts on (such as KON-12 · from its last checkpoint); what you leave out and why; and what it uses (runs, \
  drafts, the connection, an estimate of tokens). Nothing is done: once the person approves it, a turn gives the plan \
  back to you to carry out. Call it once per turn, with the whole plan; a plan you propose again replaces the one \
  that waited. Refused outside a turn that plans (not_plan_only) and for a plan with no step or more than 20 \
  (invalid_plan).";

/// The tool as the chat's `tools/list` answers.
pub fn tool() -> Tool {
  Tool::new(PROPOSE_PLAN, DESCRIPTION, schema_for_type::<ProposePlan>())
    .with_annotations(ToolAnnotations::new().read_only(false).open_world(false))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ProposePlan {
  /// The project's id, or its name: the chat's own.
  #[serde(default)]
  pub project: Option<String>,
  /// The steps in the order they would be done.
  pub steps: Vec<ProposedStep>,
  /// What the plan leaves out, and why.
  #[serde(default)]
  pub left_out: Option<String>,
  /// What the plan uses: runs, drafts, the connection and an estimate of tokens.
  #[serde(default)]
  pub uses: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ProposedStep {
  /// `run`, `draft`, `setting` or `other`.
  pub kind: ProposedKind,
  /// What will happen, in a few words (such as "Continue run #8").
  pub action: String,
  /// What it acts on (such as "KON-12 · from its last checkpoint").
  #[serde(default)]
  pub target: Option<String>,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum ProposedKind {
  Run,
  Draft,
  Setting,
  Other,
}

impl From<ProposedKind> for StepKind {
  fn from(kind: ProposedKind) -> Self {
    match kind {
      ProposedKind::Run => Self::Run,
      ProposedKind::Draft => Self::Draft,
      ProposedKind::Setting => Self::Setting,
      ProposedKind::Other => Self::Other,
    }
  }
}

/// Whether the turn under way in `scope`'s conversation plans: its person's message was sent in plan-only mode, or a
/// wait, a run's end or a day's report started it while the conversation's switch is on, or the person handed it a
/// message sent in plan-only mode (*Send now*).
pub(super) async fn planning(scope: &ChatScope) -> Result<bool, ToolError> {
  let (root, chat) = (scope.root.clone(), scope.chat);
  blocking(move || {
    let db = ProjectDb::open(&root)?;
    let switched = db.chat(chat)?.is_some_and(|chat| chat.plan_only());
    Ok(turn_plans(&db.chat_entries(chat)?, switched))
  })
  .await
}

/// Whether the turn under way among a conversation's `entries` plans, its switch `switched` as it is. Any turn that
/// thinks and plans makes it plan, so a turn kept to carry out a plan approved while the planning turn still thought
/// lets none of that turn's changes through; so does a message sent in plan-only mode that was handed to the turn
/// under way (steering, or joined), whatever that turn's own mode: from the oldest turn that thinks on, each such
/// message is the turn's.
fn turn_plans(entries: &[ChatEntry], switched: bool) -> bool {
  let Some(first) = entries
    .iter()
    .position(|entry| entry.kind.turn().is_some_and(|(_, state)| state.is_thinking()))
  else {
    return false;
  };
  entries[first..].iter().any(|entry| match &entry.kind {
    EntryKind::Person { mode, turn, .. } => {
      *mode == TurnMode::Plans && matches!(turn, TurnState::Thinking | TurnState::Steering | TurnState::Joined)
    }
    EntryKind::Wait { .. } | EntryKind::Report { .. } => {
      switched && entry.kind.turn().is_some_and(|(_, state)| state.is_thinking())
    }
    _ => false,
  })
}

/// Whether a turn thinks among a conversation's `entries`: a plan is approved only once none does, so the turn that
/// carries it out is the one the runner takes next.
fn any_thinking(entries: &[ChatEntry]) -> bool {
  entries
    .iter()
    .any(|entry| entry.kind.turn().is_some_and(|(_, state)| state.is_thinking()))
}

/// Whether the turn under way in `scope`'s conversation carries out a plan the person approved: it approves no card.
pub(super) async fn carrying_out(scope: &ChatScope) -> Result<bool, ToolError> {
  let (root, chat) = (scope.root.clone(), scope.chat);
  blocking(move || {
    let entries = ProjectDb::open(&root)?.chat_entries(chat)?;
    Ok(
      entries.iter().rev().find_map(|entry| match &entry.kind {
        EntryKind::Person { mode, turn, .. } if turn.is_thinking() => Some(*mode == TurnMode::CarriesOut),
        _ => None,
      }) == Some(true),
    )
  })
  .await
}

/// The refusal of `name`, a tool that changes something, in a turn that plans; kept in the conversation as a refused
/// action, so the person sees it.
pub(super) async fn refuse((translate, runners): (&Translate, &Runners), scope: &ChatScope, name: &str) -> ToolError {
  let error = ToolError::refused("plan_only", vec![Message::new("orchestrator.tools.planOnly")]);
  let action = ChatAction::new(name, None, keep::refusal(&error, translate));
  keep::keep(scope, (None, None), action).await;
  runners.chat_changed(&scope.root);
  error
}

/// `propose_plan` with `arguments` in `scope`: keeps the plan, in place of the one that waited, for the person.
pub(super) async fn call(
  (translate, runners): (&Translate, &Runners),
  scope: &ChatScope,
  mut arguments: JsonObject,
) -> Result<Value, ToolError> {
  super::own_project::in_own_project(scope, &mut arguments)?;
  let asked: ProposePlan =
    serde_json::from_value(Value::Object(arguments)).map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
  if !planning(scope).await? {
    return Err(ToolError::refused(
      "not_plan_only",
      vec![Message::new("orchestrator.tools.notPlanOnly")],
    ));
  }
  let steps: Vec<PlanStep> = asked
    .steps
    .iter()
    .filter_map(|step| PlanStep::new(step.kind.into(), &step.action, step.target.as_deref()))
    .collect();
  let plan = ChatPlan::new(steps, asked.left_out.as_deref(), asked.uses.as_deref())
    .map_err(|_| ToolError::refused("invalid_plan", vec![Message::new("orchestrator.tools.invalidPlan")]))?;
  let count = plan.steps().len();
  let (root, chat) = (scope.root.clone(), scope.chat);
  let seq = blocking(move || Ok(ProjectDb::open(&root)?.add_chat_plan(chat, &plan, Timestamp::now())?)).await?;
  runners.chat_changed(&scope.root);
  Ok(json!({
    "proposed": true,
    "plan": seq,
    "steps": count,
    "message": translate.text(&Message::new("orchestrator.tools.planProposed")),
  }))
}

/// Turns conversation `chat`'s *Plan only* switch on or off in the project at `root`. Blocks on SQLite.
pub fn set_plan_only_in(root: &Path, chat: ChatId, on: bool) -> Result<(), Message> {
  let kept = ProjectDb::open(root)
    .and_then(|mut db| db.set_chat_plan_only(chat, on))
    .map_err(|error| messages::store_error(&error))?;
  if kept {
    Ok(())
  } else {
    Err(Message::new("orchestrator.conversation.gone"))
  }
}

/// Dismisses the plan kept as entry `seq` in the project at `root`, while it waits. Blocks on SQLite.
pub fn dismiss_in(root: &Path, seq: i64) -> Result<(), Message> {
  let dismissed = ProjectDb::open(root)
    .and_then(|mut db| db.dismiss_chat_plan(seq, Timestamp::now()))
    .map_err(|error| messages::store_error(&error))?;
  if dismissed { Ok(()) } else { Err(taken()) }
}

/// The plan was approved, replaced or dismissed meanwhile.
pub fn taken() -> Message {
  Message::new("orchestrator.plan.taken")
}

/// A turn of the plan's conversation still thinks: the plan is approved once it ended.
pub fn busy() -> Message {
  Message::new("orchestrator.plan.busy")
}

/// Approves the plan kept as entry `seq` in the project at `root`, sent `by` the person, keeping `message` as the one
/// whose turn carries it out; refused ([`busy`]) while a turn of its conversation still thinks, and [`taken`] once it
/// no longer waits. Blocks on SQLite.
pub fn approve_in(root: &Path, seq: i64, (message, by): (ChatMessageId, Answerer)) -> Result<(), Message> {
  let approve = || -> Result<Result<(), Message>, orchester_store::StoreError> {
    let mut db = ProjectDb::open(root)?;
    let Some((chat, _)) = db.chat_plan(seq)? else {
      return Ok(Err(taken()));
    };
    if any_thinking(&db.chat_entries(chat)?) {
      return Ok(Err(busy()));
    }
    let approved = db.approve_chat_plan(seq, (message, by), Timestamp::now())?;
    Ok(approved.map(drop).ok_or_else(taken))
  };
  approve().map_err(|error| messages::store_error(&error))?
}

/// Approves the plan kept as entry `seq` of `project`, sent `by` the person, and asks the runner for the turn that
/// carries it out under `message`, with the domain tools at `tools`. A turn the runner did not take is taken back: the
/// plan waits again.
pub async fn approve(
  runners: &Runners,
  project: &OpenProject,
  (seq, message, by): (i64, ChatMessageId, Answerer),
  tools: ChatTools,
) -> Result<(), SendError> {
  if let Err(error) = runners.chat_runner(project).await {
    return Err(SendError::NotSent(not_sent(&RunRequestError::Request(error))));
  }
  let root = project.repo_root.clone();
  background::run(move || approve_in(&root, seq, (message, by)))
    .await
    .and_then(|approved| approved)
    .map_err(SendError::NotSent)?;
  runners.chat_changed(&project.repo_root);
  let Err(error) = runners.answer_in_chat(project, message, tools).await else {
    return Ok(());
  };
  if matches!(error, RunRequestError::Request(RequestError::Lost)) {
    return Err(SendError::Unconfirmed(Message::new("orchestrator.send.lost")));
  }
  let root = project.repo_root.clone();
  let reopened =
    background::run(move || ProjectDb::open(&root).and_then(|mut db| db.reopen_chat_plan(seq, message))).await;
  match reopened {
    Ok(Ok(_)) => {}
    Ok(Err(error)) => tracing::warn!(%error, "cannot take back the approval of a plan the runner did not carry out"),
    Err(error) => tracing::warn!(
      ?error,
      "cannot take back the approval of a plan the runner did not carry out"
    ),
  }
  runners.chat_changed(&project.repo_root);
  Err(SendError::NotSent(not_sent(&error)))
}
