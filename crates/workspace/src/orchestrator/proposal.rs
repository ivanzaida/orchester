//! A run the Orchestrator's agent asks for waits for a person (`docs/design/orchester-workspace.md`, "Project
//! orchestrator conversation": review before work starts), unless auto mode is on for the project
//! ([`super::tools`]). The agent's `run_task` checks that the task could run now, as Orchester's own `run_task` tool
//! checks it, and starts nothing: the conversation keeps a proposed start with the pipeline it would run, and the
//! agent is told it waits for the person. The person starts it, through the same `run_task` tool any client of the
//! app calls, or dismisses it; the person applies or dismisses a proposed change ([`super::change`]) the same way.
//! That only a person takes the start or applies the change is the window's to enforce: synthetic input never
//! reaches [`decide`].

use orchester_core::{
  orchestrator::{ActionOutcome, SentWay},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::{Value, json};

use super::{change, keep::refusal, tools::ChatScope};
use crate::{
  app_mcp::{self, Proposal, Services, ToolError, Translate},
  background,
  messages::{Message, store_error},
  runner::Runners,
};

/// What the person did with a proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
  /// Start the proposed run, or apply the proposed change, which the person was shown to widen what agents may do
  /// by `widens`: anything else is refused.
  Accept {
    widens: Vec<Message>,
  },
  Dismiss,
}

/// The agent's `run_task` with `arguments` in `scope`: the start it proposes, and what the agent is told; a refusal
/// when the task could not run now.
pub(super) async fn propose(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  arguments: &JsonObject,
) -> Result<(Value, Proposal), ToolError> {
  let key = arguments
    .get("key")
    .and_then(Value::as_str)
    .ok_or_else(|| ToolError::InvalidArguments("key: a task's key is required".to_owned()))?;
  let proposal = app_mcp::propose_run(services, translate, (scope.project.to_string(), key.to_owned())).await?;
  let waits = Message::new("orchestrator.tools.startWaits")
    .with("task", proposal.key.clone())
    .with("pipeline", proposal.start.pipeline.clone());
  let told = json!({
    "started": false,
    "waits_for_person": true,
    "task": proposal.key,
    "pipeline": proposal.start.pipeline,
    "message": translate.text(&waits),
  });
  Ok((told, proposal))
}

/// Takes the person's `decision` on the proposal `seq` of `scope`'s conversation. A start runs the task as
/// Orchester's own `run_task` tool does, and a change goes through the tool that carries it out, refused or carried
/// out as for any client, but only as it was proposed: a task, draft or run changed since, or a pipeline published
/// anew, is refused and needs a fresh proposal. A decision first claims the proposal, so a second one (another tab, a
/// second click) is refused and changes nothing. The proposal shows how it went.
pub async fn decide(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  seq: i64,
  decision: Decision,
) -> Result<(), Message> {
  let (root, chat) = (scope.root.clone(), scope.chat);
  let Decision::Accept { widens } = decision else {
    let dismissed = background::run(move || {
      ProjectDb::open(&root)?.settle_chat_action((chat, seq), ActionOutcome::Dismissed, Timestamp::now())
    })
    .await?
    .map_err(|error| store_error(&error))?;
    runners.chat_changed(&scope.root);
    return dismissed.map(drop).ok_or_else(taken);
  };
  let claimed = background::run(move || ProjectDb::open(&root)?.claim_chat_action((chat, seq), Timestamp::now()))
    .await?
    .map_err(|error| store_error(&error))?;
  let Some(action) = claimed else {
    return Err(taken());
  };
  runners.chat_changed(&scope.root);
  // A start reads as done as before; a change as the person's, apart from what the chat did itself.
  let (carried, applied) = match (action.task, action.outcome, action.change) {
    (Some(task), ActionOutcome::Proposed(start), _) => (
      app_mcp::run_proposed(services, (scope.project.to_string(), task), start.basis).await,
      ActionOutcome::Done,
    ),
    (_, ActionOutcome::Waits, Some(proposed)) => (
      change::apply(
        (services, translate),
        scope,
        &proposed,
        (&widens, SentWay::AppliedByPerson),
      )
      .await,
      ActionOutcome::Applied,
    ),
    _ => (
      Err(ToolError::InvalidArguments("nothing to carry out".to_owned())),
      ActionOutcome::Applied,
    ),
  };
  let outcome = match carried {
    Ok(_) => applied,
    Err(error) => refusal(&error, translate),
  };
  let root = scope.root.clone();
  background::run(move || ProjectDb::open(&root)?.finish_chat_action((chat, seq), outcome, Timestamp::now()))
    .await?
    .map_err(|error| store_error(&error))?;
  runners.chat_changed(&scope.root);
  Ok(())
}

/// The proposal no longer waits: another decision took it, or it was settled.
fn taken() -> Message {
  Message::new("orchestrator.proposal.taken")
}
