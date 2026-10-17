//! `send_message`: a message to the agents of a task's latest run, sent as the Conversation tab's
//! composer sends it in the run's state (`docs/design/orchester-workspace.md`, "The chat always
//! answers"): to the agent the state addresses; now or after the turn while an agent works; as the
//! answer to a question that waits, the changes a waiting plan needs, a follow-up, or instructions
//! that continue a failed or stuck run; resuming a paused run, continuing an interrupted one, running
//! an ended task again with it, or kept until a person raises the limit the run stopped at. A run no
//! agent reads any more, and that does not run again, refuses it.

use orchester_core::{
  id::{IdempotencyKey, RunId},
  run::Answerer,
};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  SentBy, Translate,
  runs::{find_run, status_now},
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  tasks::{
    conversation::{
      SendError,
      composer::ComposerMode,
      follow::{RunnerOf, Sent, Stored, follow},
      outgoing::{OutgoingError, Way, plan_changes, prepare_in, store_by_orchestrator_in, store_in},
    },
    outside::Outside,
    run::control_message,
  },
  workspace::OpenProject,
};

/// A message of the Orchestrator chat that would decide the plan a run waits on: refused, a person decides it.
pub fn plan_person_only() -> ToolError {
  ToolError::refused(
    "plan_decision_person_only",
    vec![Message::new("orchestrator.tools.planPersonOnly")],
  )
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SendMessage {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`): the latest run of its task.
  pub run: String,
  /// The message.
  pub text: String,
  /// While an agent works whose connection takes a message mid-turn: `now` (it gets the message at
  /// its next tool call) or `after_turn` (when its turn ends; the default). In every other state the
  /// run's state decides when, as in the app.
  #[serde(default)]
  pub timing: Option<SendTiming>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum SendTiming {
  Now,
  AfterTurn,
}

/// `send_message`; `by` the Orchestrator chat, the message is named the chat's in the task's conversation, and a
/// message that would decide a waiting plan is refused: plan decisions stay the person's.
pub async fn send_message(
  services: &Services,
  args: SendMessage,
  (translate, by): (&Translate, SentBy),
) -> Result<Value, ToolError> {
  let reading = services.clone();
  let (project, run) = (args.project.clone(), args.run.clone());
  let (project, prepared) = blocking(move || {
    let project = reading.project(&project)?;
    let named = find_run(&orchester_store::ProjectDb::open(&project.repo_root)?, &run)?;
    // Automation reads the run as stored: the runner it asks starts if the project has none.
    let prepared = prepare_in(&project.repo_root, (named.task(), Some(named.id())), (false, None)).map_err(refusal)?;
    Ok((project, prepared))
  })
  .await?;
  let run = prepared.run().expect("the run named is the task's latest");
  if let ComposerMode::PlanChanges { .. } = prepared.mode {
    if by == SentBy::Orchestrator {
      return Err(plan_person_only());
    }
    let command = plan_changes(&prepared, &args.text, (Answerer::Automation, None)).map_err(refusal)?;
    services
      .runners
      .control_run_in(&project, command)
      .await
      .map_err(|error| ToolError::runner(&error, vec![control_message(&error)]))?;
    return reply(&project, run, ("request_plan_changes", Sent::Taken), translate).await;
  }
  let key = IdempotencyKey::generate();
  let way = match (&prepared.mode, args.timing) {
    (ComposerMode::Steer(working), Some(SendTiming::Now)) if working.mid_turn => Way::Primary,
    (ComposerMode::Steer(working), _) if working.mid_turn => Way::Second,
    _ => Way::Primary,
  };
  let (root, text) = (project.repo_root.clone(), args.text.clone());
  let (prepared, then) = blocking(move || {
    let store = match by {
      SentBy::Client => store_in,
      SentBy::Orchestrator => store_by_orchestrator_in,
    };
    let (then, _) = store(&root, &prepared, (key, way), &text).map_err(refusal)?;
    Ok((prepared, then))
  })
  .await?;
  // An open Conversation of the task reads the stored message in place.
  services.changes.record(project.project.id(), Outside::Tasks);
  let pipeline = prepared
    .detail
    .run
    .as_ref()
    .map(|view| view.pipeline.clone())
    .unwrap_or_default();
  let stored = Stored {
    root: &project.repo_root,
    task: prepared.detail.task.id(),
    run,
    key,
    pipeline: &pipeline,
  };
  let runner = (&services.runners, &services.servers, RunnerOf::Project(&project));
  let sent = follow(runner, &stored, then).await;
  reply(&project, run, (action(&prepared.mode, way), sent), translate).await
}

/// Why the message was refused, as the tool says it.
pub fn refusal(error: OutgoingError) -> ToolError {
  let reason = error.message();
  match error {
    OutgoingError::Store(error) | OutgoingError::Send(SendError::Store(error)) => error.into(),
    OutgoingError::NotLatestRun(_) => ToolError::refused("not_latest_run", vec![reason]),
    OutgoingError::Closed(_) => ToolError::refused("no_agent_reads", vec![reason]),
    OutgoingError::Send(SendError::Blank) => ToolError::refused("blank_message", vec![reason]),
    OutgoingError::Changed
    | OutgoingError::NoSecondWay
    | OutgoingError::NoPlan
    | OutgoingError::Send(SendError::InvalidRecovery | SendError::NotUndelivered | SendError::NotReadable) => {
      ToolError::refused("not_sent", vec![reason])
    }
  }
}

/// What the composer's action is in the run's state, as the result names it.
fn action(mode: &ComposerMode, way: Way) -> &'static str {
  match mode {
    ComposerMode::Steer(working) if working.mid_turn && way == Way::Primary => "send_now",
    ComposerMode::Steer(_) => "send_after_turn",
    ComposerMode::Queue(_) => "queue",
    ComposerMode::Answer { .. } => "answer",
    ComposerMode::PlanChanges { .. } => "request_plan_changes",
    ComposerMode::FollowUp => "follow_up",
    ComposerMode::Continue => "continue",
    ComposerMode::Resume { interrupted: false } => "resume",
    ComposerMode::Resume { interrupted: true } => "continue_interrupted",
    ComposerMode::RaiseLimit => "raise_limit",
    ComposerMode::NotStarted => "when_run_starts",
    ComposerMode::RunAgain => "run_again",
    ComposerMode::Closed => "closed",
  }
}

/// The result: what the message did, why it waits when it does, and the run as stored now.
async fn reply(
  project: &OpenProject,
  run: RunId,
  (action, sent): (&str, Sent),
  translate: &Translate,
) -> Result<Value, ToolError> {
  let (outcome, reasons) = match sent {
    Sent::Taken => ("taken", Vec::new()),
    Sent::Queued(reason) => ("queued", vec![translate.text(&reason)]),
    Sent::AwaitsLimit => (
      "awaits_limit",
      vec![translate.text(&Message::new("runs.limit.notRaised"))],
    ),
  };
  let mut result = status_now(project, run).await?;
  result["action"] = json!(action);
  result["outcome"] = json!(outcome);
  result["reasons"] = json!(reasons);
  Ok(result)
}
