//! `send_message`: a message to the agents of a task's latest run, sent as the Conversation tab's
//! composer sends it in the run's state (`docs/design/orchester-workspace.md`, "The chat always
//! answers"): to the agent the state addresses; now or after the turn while an agent works; as the
//! answer to a question that waits, the changes a waiting plan needs, a follow-up, or instructions
//! that continue a failed or stuck run; resuming a paused run, continuing an interrupted one, running
//! an ended task again with it, or kept until a person raises the limit the run stopped at. A run no
//! agent reads any more, and that does not run again, refuses it.

use orchester_core::{
  conversation::{Recipient, Timing},
  id::{IdempotencyKey, RunId},
  run::{PlanDecision, ReviewNote},
};
use orchester_ipc::Command;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Translate,
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
      Conversation, SendError,
      composer::{ComposerMode, Then},
      follow::{RunnerOf, Sent, Stored, follow},
      send_in,
    },
    detail::TaskDetail,
    outside::Outside,
    plan::PlanReview,
    run::control_message,
  },
  workspace::OpenProject,
};

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

/// What the run's state makes of the message, read from what is stored.
struct Prepared {
  /// The task's latest run, which the message is for.
  run: RunId,
  detail: TaskDetail,
  conversation: Conversation,
  mode: ComposerMode,
  recipient: Recipient,
  timing: Timing,
}

pub async fn send_message(services: &Services, args: SendMessage, translate: &Translate) -> Result<Value, ToolError> {
  let reading = services.clone();
  let (project, run) = (args.project.clone(), args.run.clone());
  let (project, prepared) = blocking(move || {
    let project = reading.project(&project)?;
    let named = find_run(&orchester_store::ProjectDb::open(&project.repo_root)?, &run)?;
    let prepared = prepare(&project, named.id(), named.task(), args.timing)?;
    Ok((project, prepared))
  })
  .await?;
  let run = prepared.run;
  if let ComposerMode::PlanChanges { .. } = prepared.mode {
    request_plan_changes(services, &project, &prepared, &args.text).await?;
    return reply(&project, run, ("request_plan_changes", Sent::Taken), translate).await;
  }
  let then = prepared.mode.then().ok_or_else(|| closed(&prepared.conversation))?;
  let key = IdempotencyKey::generate();
  store(&project, &prepared, (key, then), args.text).await?;
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
  reply(&project, run, (action(&prepared), sent), translate).await
}

/// The task's conversation and the composer's mode for the message, with whom it goes to; refused
/// unless `run` is the task's latest.
fn prepare(
  project: &OpenProject,
  run: RunId,
  task: orchester_core::id::TaskId,
  timing: Option<SendTiming>,
) -> Result<Prepared, ToolError> {
  let detail = TaskDetail::load_in(&project.repo_root, task)?;
  let latest = detail.run.as_ref().map(crate::tasks::detail::RunView::id);
  if latest != Some(run) {
    let reason = match &detail.run {
      Some(latest) => {
        Message::new("conversation.errors.notLatestRun").with("run", latest.run.number().get().to_string())
      }
      None => Message::new("runs.unreadable.rejected"),
    };
    return Err(ToolError::refused("not_latest_run", vec![reason]));
  }
  // Automation reads the run as stored: the runner it asks starts if the project has none.
  let conversation = Conversation::of(&detail, false, None);
  let run_view = detail.run.as_ref();
  let recipient = ComposerMode::of(&conversation, run_view, Recipient::Orchestrator).default_recipient(&conversation);
  let mode = ComposerMode::of(&conversation, run_view, recipient);
  let timing = match (&mode, timing) {
    (ComposerMode::Steer(working), Some(SendTiming::Now)) if working.mid_turn => Timing::Now,
    _ => Timing::AfterTurn,
  };
  Ok(Prepared {
    run,
    detail,
    conversation,
    mode,
    recipient,
    timing,
  })
}

/// Stores the message as the composer stores it: for the task's next run when it runs again, with
/// the failure it continues after when it continues the run.
async fn store(
  project: &OpenProject,
  prepared: &Prepared,
  (key, then): (IdempotencyKey, Then),
  text: String,
) -> Result<(), ToolError> {
  let root = project.repo_root.clone();
  let task = prepared.detail.task.id();
  let run = Some(prepared.run).filter(|_| !then.for_next_run());
  let recovery_after = prepared
    .conversation
    .recovery_after
    .filter(|_| prepared.mode == ComposerMode::Continue);
  let (recipient, timing) = (prepared.recipient, prepared.timing);
  blocking(move || {
    send_in(&root, key, (task, run, recovery_after), (recipient, timing), &text).map_err(|error| {
      let reason = error.message();
      match error {
        SendError::Store(error) => error.into(),
        SendError::Blank => ToolError::refused("blank_message", vec![reason]),
        SendError::InvalidRecovery | SendError::NotUndelivered => ToolError::refused("not_sent", vec![reason]),
      }
    })?;
    Ok(())
  })
  .await
}

/// *Request changes* with the message as the changes the waiting plan needs.
async fn request_plan_changes(
  services: &Services,
  project: &OpenProject,
  prepared: &Prepared,
  text: &str,
) -> Result<(), ToolError> {
  let review = prepared
    .detail
    .run
    .as_ref()
    .and_then(PlanReview::of)
    .expect("a plan waits in the plan's mode");
  let note = ReviewNote::new(text)
    .map_err(|_| ToolError::refused("blank_message", vec![Message::new("conversation.errors.blank")]))?;
  let waiting = review.waiting();
  let command = Command::DecidePlan {
    run: review.run,
    execution: waiting.execution,
    version: waiting.version,
    decision: PlanDecision::ChangesRequested { note },
  };
  services
    .runners
    .control_run_in(project, command)
    .await
    .map_err(|error| ToolError::runner(&error, vec![control_message(&error)]))
}

/// No agent reads the message, and the task does not run again: why, as the closed composer says it.
fn closed(conversation: &Conversation) -> ToolError {
  ToolError::refused("no_agent_reads", vec![Message::new(conversation.state.closed_reason())])
}

/// What the composer's action is in the run's state, as the result names it.
fn action(prepared: &Prepared) -> &'static str {
  match &prepared.mode {
    ComposerMode::Steer(_) if prepared.timing == Timing::Now => "send_now",
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
