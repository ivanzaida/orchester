//! The Orchestrator's own decisions with their reasons (`orchester_core::orchestrator::decision`,
//! `docs/plan/orchestrator-autonomy/OA-22-…`). In a turn Orchester started by itself, a call that is a judgement
//! ([`JUDGED`]) carries a one-sentence `reason` and is refused `reason_required` without one; done, it is kept as a
//! decision of the project beside its action. A tool request's answer has its reason already
//! ([`super::approval_turn`]) and is kept the same way. A turn a person's message started makes no decision: they
//! asked, and a reason given there is only dropped before the app's tool is called.

use std::{path::Path, sync::Arc};

use orchester_core::{
  orchestrator::decision::{Decision, NewDecision, Reason, Review, Undo},
  time::Timestamp,
};
use orchester_store::{DecisionsRead, ProjectDb, StoreError};
use rmcp::model::{JsonObject, Tool};
use serde_json::{Value, json};

use super::{
  module_turn::{DELIVER_MODULE, START_ACCEPTANCE},
  parking::PARK_TASK,
  tools::{ANSWER_QUESTION, CONTINUE_RUN, ChatScope, RUN_TASK},
};
use crate::{
  app_mcp::{ToolError, find_run, find_task},
  messages::{self, Message},
  tasks::recorded::task_of,
};

const REASON: &str = "reason";
const DECIDE_PLAN: &str = "decide_plan";

/// The tools whose call is a judgement of the chat's own when nobody asked for it: how a stopped run goes on (an
/// acceptance, a return, a retry, a raise), an answer to an agent's question, a decision on a plan, and a module's
/// acceptance and delivery.
const JUDGED: [&str; 5] = [
  CONTINUE_RUN,
  ANSWER_QUESTION,
  DECIDE_PLAN,
  START_ACCEPTANCE,
  DELIVER_MODULE,
];

/// The tools that take a reason: the judgements, and `run_task`, whose reason is required only when it runs a task
/// again ([`super::led_start`]).
const REASONED: [&str; 6] = [
  CONTINUE_RUN,
  ANSWER_QUESTION,
  DECIDE_PLAN,
  RUN_TASK,
  START_ACCEPTANCE,
  DELIVER_MODULE,
];

/// `tool`, given the chat's `reason` when it is one of [`REASONED`].
pub(super) fn with_reason(tool: &mut Tool) {
  if !REASONED.contains(&tool.name.as_ref()) {
    return;
  }
  let mut schema = (*tool.input_schema).clone();
  if let Some(Value::Object(properties)) = schema.get_mut("properties") {
    properties.insert(
      REASON.to_owned(),
      json!({
        "type": "string",
        "description": "Why you decide so, in one sentence. Required in a turn nobody asked you for (reason_required without it): it is kept with the decision for the person to review.",
      }),
    );
  }
  tool.input_schema = Arc::new(schema);
}

/// The chat's reason, and `arguments` without it as the app's tool takes them; a tool that takes none is left as it
/// is. A reason that is no text is the call's error.
pub(super) fn split(name: &str, arguments: &JsonObject) -> Result<(Option<String>, JsonObject), ToolError> {
  let mut rest = arguments.clone();
  if !REASONED.contains(&name) {
    return Ok((None, rest));
  }
  match rest.remove(REASON) {
    None | Some(Value::Null) => Ok((None, rest)),
    Some(Value::String(reason)) => Ok((Some(reason), rest)),
    Some(_) => Err(ToolError::InvalidArguments(format!("{REASON}: one sentence of text"))),
  }
}

/// The refusal of a call of `name` that needs a reason and has none.
pub(super) fn reason_required(name: &str) -> ToolError {
  ToolError::refused(
    "reason_required",
    vec![Message::new("orchestrator.tools.reasonRequired").with("tool", name.to_owned())],
  )
}

/// The refusal of the judgement `name` with `arguments` in a turn nobody asked for when `reason` is no reason
/// (`reason_required`); `None` when it has one, is no judgement, or a person's message started the turn.
pub(super) fn refused(
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
  reason: Option<&str>,
) -> Option<ToolError> {
  if !scope.on_its_own || !judged(name, arguments) || reason.and_then(Reason::new).is_some() {
    return None;
  }
  Some(reason_required(name))
}

/// Whether the call is a judgement: resuming a paused run is not, it decides nothing.
fn judged(name: &str, arguments: &JsonObject) -> bool {
  JUDGED.contains(&name) && !(name == CONTINUE_RUN && choice(arguments) == Some("resume"))
}

fn choice(arguments: &JsonObject) -> Option<&str> {
  arguments.get("choice")?.get("kind")?.as_str()
}

/// Keeps what the chat did through `name` with `arguments`, in a turn nobody asked for, as a decision with `reason`,
/// on the task `task` names (else the one the call names); a reason that is none keeps nothing. One that cannot be
/// kept is logged: the action itself is in the conversation either way.
pub(super) async fn keep(
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
  (task, reason): (Option<String>, &str),
) {
  let Some(reason) = Reason::new(reason) else {
    return;
  };
  let text = |name: &str| arguments.get(name).and_then(Value::as_str).map(str::to_owned);
  let (task, run) = (task.or_else(|| text("key")).or_else(|| text("task")), text("run"));
  let undo = match name {
    CONTINUE_RUN if choice(arguments) == Some("accept") => Some(Undo::ReturnToStep),
    PARK_TASK => Some(Undo::GiveBack),
    _ => None,
  };
  let (root, chat, tool) = (scope.root.clone(), scope.chat, name.to_owned());
  let kept = tokio::task::spawn_blocking(move || {
    let mut db = ProjectDb::open(&root)?;
    let run = run.and_then(|named| find_run(&db, &named).ok());
    let task = match (task, &run) {
      (Some(named), _) => Some(find_task(&db, &named).map_or(named, |task| task.key().to_string())),
      (None, Some(run)) => task_of(&db, run.task())?.map(|task| task.key().to_string()),
      (None, None) => None,
    };
    let decision = NewDecision {
      chat,
      tool,
      task,
      run: run.map(|run| run.id()),
      reason,
      undo,
      at: Timestamp::now(),
    };
    db.keep_decision(&decision)
  })
  .await;
  match kept {
    Ok(Ok(_)) => {}
    Ok(Err(error)) => tracing::error!(%chat, %error, "cannot keep a decision of the Orchestrator"),
    Err(error) => tracing::error!(%chat, %error, "keeping a decision of the Orchestrator did not finish"),
  }
}

/// The decisions `read` wants of the project at `root`: the open ones first, then the newest first. Blocks on SQLite.
pub fn decisions_in(root: &Path, read: &DecisionsRead) -> Result<Vec<Decision>, Message> {
  let decisions = || -> Result<Vec<Decision>, StoreError> { ProjectDb::open(root)?.decisions(read) };
  decisions().map_err(|error| messages::store_error(&error))
}

/// The person reviews decision `number` of the project at `root` as `review` (seen, or overruled); whether it
/// changed: one reviewed that far already stays as it is. The person's alone: no tool of the app or the chat calls
/// it. Blocks on SQLite.
pub fn review_in(root: &Path, number: i64, review: Review) -> Result<bool, Message> {
  let reviewed = || -> Result<bool, StoreError> { ProjectDb::open(root)?.review_decision(number, review) };
  reviewed().map_err(|error| messages::store_error(&error))
}
