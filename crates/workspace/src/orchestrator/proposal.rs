//! A run the Orchestrator's agent asks for waits for a person (`docs/design/orchester-workspace.md`, "Project
//! orchestrator conversation": review before work starts), unless auto mode is on for the project
//! ([`super::tools`]). The agent's `run_task` checks that the task could run now, as Orchester's own `run_task` tool
//! checks it, and starts nothing: the conversation keeps a proposed start with the pipeline it would run, and the
//! agent is told it waits for the person. The person starts it, through the same `run_task` tool any client of the
//! app calls, or dismisses it; the person applies or dismisses a proposed change ([`super::change`]) the same way.
//! That only a person takes the start or applies the change is the window's to enforce: synthetic input never
//! reaches [`decide`].

use orchester_core::orchestrator::{ActionOutcome, ChatAction, EntryKind};
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
  /// Start the proposed run, or apply the proposed change.
  Accept,
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
/// anew, is refused and needs a fresh proposal. The proposal shows how it went; one that no longer waits is left as
/// it is.
pub async fn decide(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  seq: i64,
  decision: Decision,
) -> Result<(), Message> {
  let outcome = match decision {
    Decision::Dismiss => ActionOutcome::Dismissed,
    Decision::Accept => {
      let Some(action) = waiting(scope, seq).await? else {
        return Ok(());
      };
      let carried = match (action.task, action.outcome) {
        (Some(task), ActionOutcome::Proposed(start)) => {
          app_mcp::run_proposed(services, (scope.project.to_string(), task), start.basis).await
        }
        (_, ActionOutcome::ProposedChange(proposed)) => change::apply((services, translate), scope, &proposed).await,
        _ => return Ok(()),
      };
      match carried {
        Ok(_) => ActionOutcome::Done,
        Err(error) => refusal(&error, translate),
      }
    }
  };
  let (root, chat) = (scope.root.clone(), scope.chat);
  background::run(move || ProjectDb::open(&root)?.settle_chat_action((chat, seq), outcome))
    .await?
    .map_err(|error| store_error(&error))?;
  runners.chat_changed(&scope.root);
  Ok(())
}

/// The action `seq` of `scope`'s conversation while it waits for a person.
async fn waiting(scope: &ChatScope, seq: i64) -> Result<Option<ChatAction>, Message> {
  let (root, chat) = (scope.root.clone(), scope.chat);
  let entries = background::run(move || ProjectDb::open(&root)?.chat_entries(chat))
    .await?
    .map_err(|error| store_error(&error))?;
  Ok(entries.into_iter().find_map(|entry| match entry.kind {
    EntryKind::Action(action) if entry.seq == seq && action.outcome.is_proposed() => Some(action),
    _ => None,
  }))
}
