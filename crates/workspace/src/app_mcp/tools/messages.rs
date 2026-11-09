//! `send_message`: a message to the agents of a task's latest run, sent as the Conversation tab's
//! composer sends it in the run's state (`docs/design/orchester-workspace.md`, "The chat always
//! answers"): to the agent the state addresses; now or after the turn while an agent works; as the
//! answer to a question that waits, the changes a waiting plan needs, a follow-up, or instructions
//! that continue a failed or stuck run; resuming a paused run, continuing an interrupted one, running
//! an ended task again with it, or kept until a person raises the limit the run stopped at. A run no
//! agent reads any more, and that does not run again, refuses it.

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  run::Run,
};
use orchester_store::StoreError;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  SentBy, Translate,
  runs::{find_run, status_now},
  team,
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  readiness::NotStarted,
  tasks::{
    conversation::{
      SendError,
      composer::{ComposerMode, Then},
      follow::{RunnerOf, Sent, StartsWork, Stored, TeamThen, follow, follow_team},
      outgoing::{OutgoingError, Prepared, Way, plan_changes, prepare_in, store_answer_in, store_by_orchestrator_in},
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

/// How a message reaches the run: through this computer's own runner and claims, or, for a team task's run, under the
/// claim its work started with.
#[derive(Clone, Copy)]
enum Reach {
  Local,
  Team(TeamThen),
}

/// How the message sent `way` in `mode` reaches run `run`. A team task's run (told by the run itself: a local task's
/// run in a team-linked project is not one) the team server fenced takes no message at all, in any state, and is not
/// started again or continued from here: refused before the message is stored.
fn reach_checked(
  db: &orchester_store::ProjectDb,
  (mode, way): (&ComposerMode, Way),
  run: &Run,
) -> Result<Reach, ToolError> {
  if !team::is_team_run(db, run)? {
    return Ok(Reach::Local);
  }
  team::refuse_fenced(db, run.id())?;
  // A mode with no way of sending this way stores nothing, and a plan's changes go to the runner as a decision: nothing
  // follows from here.
  let Some(then) = way.then_in(mode) else {
    return Ok(Reach::Local);
  };
  TeamThen::try_from(then)
    .map(Reach::Team)
    .map_err(|starts| match starts {
      StartsWork::RunAgain => team_run_again(),
      StartsWork::ContinueInterrupted => team::team_only_on_server("send_message"),
    })
}

/// A message that would run a team task again: its work starts only through the team server.
fn team_run_again() -> ToolError {
  ToolError::refused(
    "team_run_again_unsupported",
    vec![Message::new("appMcp.errors.teamRunAgain")],
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
  /// run's state decides when, as in the app. Automation's message sent `now` that the agent's CLI does
  /// not take mid-turn is queued for when its turn ends (the conversation shows it `queued`); a
  /// person's is kept for them to send after the turn (`not_delivered`).
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
  (translate, by, expected): (&Translate, SentBy, Option<&str>),
) -> Result<Value, ToolError> {
  let reading = services.clone();
  let (project, run) = (args.project.clone(), args.run.clone());
  let (timing, checked) = (args.timing, expected.map(str::to_owned));
  let (project, prepared, reach) = blocking(move || {
    let project = reading.project(&project)?;
    let db = orchester_store::ProjectDb::open(&project.repo_root)?;
    let named = find_run(&db, &run)?;
    // Automation reads the run as stored: the runner it asks starts if the project has none.
    let prepared = prepare_in(&project.repo_root, (named.task(), Some(named.id())), (false, None))
      .map_err(|error| prepare_refusal(&db, error))?;
    // Checked first: a run that moved on since is refused as changed, and one that now waits on a plan would take the
    // message as its plan's changes.
    as_checked(
      &prepared,
      way_of(&prepared.mode, timing),
      (checked.as_deref(), named.number().get()),
    )?;
    let reach = reach_checked(&db, (&prepared.mode, way_of(&prepared.mode, timing)), &named)?;
    if let Some(kind) = prepared.decision_kind(Way::Primary) {
      // The Orchestrator's plan path below retains its stricter refusal.
      if !matches!(by, SentBy::Person(_)) && !matches!(prepared.mode, ComposerMode::PlanChanges { .. }) {
        kind.require(&reading.workspace)?;
      }
    }
    Ok((project, prepared, reach))
  })
  .await?;
  let run = prepared.run().expect("the run named is the task's latest");
  let way = way_of(&prepared.mode, args.timing);
  if let ComposerMode::PlanChanges { .. } = prepared.mode {
    if let SentBy::Orchestrator(_) = by {
      return Err(plan_person_only());
    }
    let workspace = services.workspace.clone();
    if !matches!(by, SentBy::Person(_)) {
      blocking(move || crate::decisions::DecisionKind::Plan.require(&workspace)).await?;
    }
    let (root, text, answerer) = (project.repo_root.clone(), args.text.clone(), by.answerer());
    let command = blocking(move || plan_changes(&root, &prepared, &text, (answerer, None)).map_err(refusal)).await?;
    services
      .runners
      .control_run_in(&project, command)
      .await
      .map_err(|error| ToolError::runner(&error, vec![control_message(&error)]))?;
    return reply(&project, run, ("request_plan_changes", Sent::Taken), translate).await;
  }
  // A message that runs the task again starts a run, which a pipeline that is not ready here cannot: refused before the
  // message is stored, so a client that sends it again once the person fixed what is missing stores it once.
  if matches!(prepared.mode, ComposerMode::RunAgain) {
    ready_to_run_again(services, &project, prepared.detail.task.id()).await?;
  }
  let key = IdempotencyKey::generate();
  let (root, text) = (project.repo_root.clone(), args.text.clone());
  let (prepared, then, message) = blocking(move || {
    // An MCP client and the Orchester chat attach no files: they only read the ones people attached.
    let stored = match by.chat() {
      None => store_answer_in(&root, &prepared, (key, way), (&text, &[], None), by.answerer()),
      Some(chat) => store_by_orchestrator_in(&root, &prepared, (key, way), (&text, chat, by.answerer())),
    };
    let (then, message) = stored.map_err(refusal)?;
    Ok((prepared, then, message.id()))
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
  let sent = followed(services, (&project, &stored), then, reach).await;
  let mut result = reply(&project, run, (action(&prepared.mode, way), sent), translate).await?;
  result["message"] = json!({ "id": message.to_string(), "attachments": [] });
  Ok(result)
}

/// What follows the stored message. What became not ready between the check and the start leaves it queued, as the
/// window's composer leaves it: the reply says it is queued, and why.
async fn followed(
  services: &Services,
  (project, stored): (&OpenProject, &Stored<'_>),
  then: Then,
  reach: Reach,
) -> Sent {
  let runner = RunnerOf::Project(project);
  if let Reach::Team(team_then) = reach {
    return follow_team((&services.runners, runner), stored, team_then).await;
  }
  let servers = (&services.runners, &services.servers, runner);
  follow(servers, (&services.workspace, &services.vault), stored, then).await
}

/// `Ok` when the pipeline of `task` is ready to start here; else the parts it lacks, or why that could not be read.
async fn ready_to_run_again(services: &Services, project: &OpenProject, task: TaskId) -> Result<(), ToolError> {
  match services
    .workspace
    .ready_to_start(&project.repo_root, &services.vault, task)
    .await
  {
    Ok(()) => Ok(()),
    Err(NotStarted::NotReadyHere(refusal)) => Err(ToolError::NotReadyHere(refusal)),
    Err(NotStarted::Unreadable(message)) => Err(ToolError::refused("readiness_unreadable", vec![message])),
  }
}

/// [`refusal`] of `error`, which `prepare_in` raised for a run of `db`'s project. A run whose task no record defines is
/// a team task's run that kept no inputs: its task is unrecorded, not missing.
fn prepare_refusal(db: &orchester_store::ProjectDb, error: OutgoingError) -> ToolError {
  match error {
    OutgoingError::Store(StoreError::TaskNotFound(task)) => team::task_missing(db, task).unwrap_or_else(|error| error),
    error => refusal(error),
  }
}

/// Why the message was refused, as the tool says it.
pub fn refusal(error: OutgoingError) -> ToolError {
  let reason = error.message();
  match error {
    OutgoingError::Store(error) | OutgoingError::Send(SendError::Store(error)) => error.into(),
    OutgoingError::NotLatestRun(_) => ToolError::refused("not_latest_run", vec![reason]),
    OutgoingError::Closed(_) => ToolError::refused("no_agent_reads", vec![reason]),
    OutgoingError::Send(SendError::Blank) => ToolError::refused("blank_message", vec![reason]),
    OutgoingError::Send(SendError::NotAllowed) => ToolError::refused("automation_not_allowed", vec![reason]),
    OutgoingError::Send(SendError::Attachments(_)) => ToolError::refused("attachments_refused", vec![reason]),
    OutgoingError::Changed
    | OutgoingError::NoSecondWay
    | OutgoingError::NoPlan
    | OutgoingError::Send(
      SendError::InvalidRecovery | SendError::NotUndelivered | SendError::NotReadable | SendError::Skill(_),
    ) => ToolError::refused("not_sent", vec![reason]),
  }
}

/// Refused unless the message does what it was checked or proposed as (`expected`, the Orchestrator's): one whose run
/// (`#number`) moved on since would do something else (`changed_since_proposed`). None of the chat's messages is
/// checked as a plan's changes, so a run that waits on a plan refuses every one.
fn as_checked(prepared: &Prepared, way: Way, (expected, number): (Option<&str>, u32)) -> Result<(), ToolError> {
  let Some(expected) = expected else {
    return Ok(());
  };
  if action(&prepared.mode, way) == expected {
    return Ok(());
  }
  Err(ToolError::refused(
    "changed_since_proposed",
    vec![Message::new("appMcp.errors.messageChanged").with("run", number.to_string())],
  ))
}

/// How a message sent with `timing` in `mode` goes: as the composer's first way, or after the turn of an agent that
/// takes messages mid-turn.
fn way_of(mode: &ComposerMode, timing: Option<SendTiming>) -> Way {
  match (mode, timing) {
    (ComposerMode::Steer(working), Some(SendTiming::Now)) if working.mid_turn => Way::Primary,
    (ComposerMode::Steer(working), _) if working.mid_turn => Way::Second,
    _ => Way::Primary,
  }
}

/// What a message sent with `timing` in `mode` does, as the result names it ([`action`]): what the Orchestrator's
/// message is checked or proposed as, and sent only as.
pub(crate) fn sent_as(mode: &ComposerMode, timing: Option<SendTiming>) -> &'static str {
  action(mode, way_of(mode, timing))
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
  let started = match &sent {
    Sent::Started(started) => Some(*started),
    Sent::Taken | Sent::Queued(_) | Sent::NotReady(_) | Sent::AwaitsLimit => None,
  };
  let (outcome, reasons) = match sent {
    Sent::Taken | Sent::Started(_) => ("taken", Vec::new()),
    Sent::Queued(reason) => ("queued", vec![translate.text(&reason)]),
    Sent::NotReady(unreadable) => ("queued", vec![translate.text(&unreadable.short())]),
    Sent::AwaitsLimit => (
      "awaits_limit",
      vec![translate.text(&Message::new("runs.limit.notRaised"))],
    ),
  };
  let mut result = status_now(project, run).await?;
  result["action"] = json!(action);
  result["outcome"] = json!(outcome);
  result["reasons"] = json!(reasons);
  if let Some(started) = started {
    // The run the message ran the task again with.
    result["started_run"] = json!({ "id": started.to_string() });
  }
  Ok(result)
}
