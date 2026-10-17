//! The domain tools the Orchestrator's agent gets (`docs/design/orchestrator-toolset.md`): a part of Orchester's own
//! MCP tools ([`crate::app_mcp`]). Each call goes through the same tool an MCP client of the app calls, so it is
//! refused or carried out exactly as for that client: what only a person decides stays refused (`person_only`),
//! approvals and plan decisions stay with people and runs start through the scheduling checks. It reads every
//! project; a change that names another project is refused before it reaches the tool, and a change is given the
//! chat's project by its id, never by a name. Publishing a draft, canceling, merging a run, answering an agent's
//! question and sending a run's agents a message wait for the person ([`super::change`]) unless the person's switch
//! lets the chat do it itself (publishing, merging, answering; never canceling), and so does starting a run unless
//! auto mode is on for the project ([`super::proposal`]). What the agent changed, or tried to, is kept in its
//! conversation as an action.

use std::path::PathBuf;

use orchester_core::{
  id::{ChatId, ProjectId, RunId},
  orchestrator::{
    ActionOutcome, ChatAction,
    change::{ProposedChange, Publishing},
  },
};
use orchester_store::ProjectDb;
use rmcp::model::{JsonObject, Tool};
use serde_json::Value;

use super::{
  blocked, change, duplicate,
  keep::{self, HOLD_QUEUE, Named},
  proposal,
};
use crate::{
  app_mcp::{self, Services, ToolError, Translate, blocking},
  messages::Message,
  runner::Runners,
  workspace::OrchestratorConsent,
};

/// The tools of the chat, by name: reading every project's work, setup and tasks' conversations, task drafts and
/// drafts of what agents run on, pausing and resuming a run, holding and releasing new starts, and what waits for the
/// person or the person's switch: starting a run, publishing a draft, canceling and merging a run, answering an
/// agent's question and sending a run's agents a message. Approvals, plan decisions, project setup, connections,
/// MCP servers, the environment and what removes work are not among them. A name whose tool the app does not have
/// (yet) is not listed ([`listed`]).
pub const TOOLS: [&str; 52] = [
  "list_projects",
  "get_project",
  "list_tasks",
  "get_task",
  "create_task",
  "update_task",
  "list_runs",
  "get_run",
  "get_changes",
  "get_conversation",
  "run_task",
  "pause_run",
  "resume_run",
  "cancel_run",
  "merge_run",
  "answer_question",
  "send_message",
  "list_approvals",
  "list_questions",
  "get_plan",
  "hold_queue",
  "release_queue",
  "get_auto_mode",
  "list_pipelines",
  "get_pipeline",
  "save_pipeline_draft",
  "create_pipeline",
  "duplicate_pipeline",
  "publish_pipeline",
  "list_profiles",
  "get_profile",
  "save_profile_draft",
  "publish_profile",
  "list_instructions",
  "get_instruction",
  "save_instruction_draft",
  "publish_instruction",
  "list_skills",
  "get_skill",
  "save_skill_draft",
  "publish_skill",
  "list_tools",
  "get_tool",
  "save_tool_draft",
  "publish_tool",
  "list_secrets",
  "list_connections",
  "list_accounts",
  "list_models",
  "get_usage",
  "list_pull_requests",
  "get_pull_request",
];

/// The tool whose call starts a run, or proposes it.
pub(super) const RUN_TASK: &str = "run_task";
/// The tool whose calls always wait for the person.
pub(super) const CANCEL_RUN: &str = "cancel_run";
/// The tool whose calls wait for the person unless *Let the Orchestrator merge runs* is on.
pub(super) const MERGE_RUN: &str = "merge_run";
/// The tools whose calls wait for the person unless *Let the Orchestrator answer agents* is on.
pub(super) const ANSWER_QUESTION: &str = "answer_question";
pub(super) const SEND_MESSAGE: &str = "send_message";
/// The tool whose copy the chat saves as a draft before it publishes it by the publication rule.
pub(super) const DUPLICATE_PIPELINE: &str = "duplicate_pipeline";

/// The chat's tools as `tools/list` answers: those of [`TOOLS`] the app has, each told how it works in the chat.
pub fn listed() -> Vec<Tool> {
  app_mcp::listed()
    .into_iter()
    .chain(app_mcp::setup::listed())
    .filter(|tool| TOOLS.contains(&tool.name.as_ref()))
    .map(|mut tool| {
      if let Some(told) = in_chat(&tool) {
        tool.description = Some(told.into());
      }
      tool
    })
    .collect()
}

/// What the agent reads of a tool that works otherwise in the chat; `None` for one that works as for any client.
fn in_chat(tool: &Tool) -> Option<String> {
  let name = tool.name.as_ref();
  let own = tool.description.as_deref().unwrap_or_default();
  let first = match name {
    RUN_TASK => return Some(PROPOSE_RUN.to_owned()),
    DUPLICATE_PIPELINE => DUPLICATE_AS_DRAFT,
    CANCEL_RUN => PROPOSE_CHANGE,
    MERGE_RUN => PROPOSE_MERGE,
    ANSWER_QUESTION | SEND_MESSAGE => PROPOSE_ANSWER,
    _ if Publishing::of_tool(name).is_some() => PROPOSE_PUBLISH,
    _ => return None,
  };
  Some(format!("{first} {own}"))
}

/// What the agent reads of `run_task`: here it proposes the start, unless auto mode is on.
const PROPOSE_RUN: &str = "Proposes running a task to the person: checks that the task could run now (a draft is \
  queued and started, a queued task starts, an ended one runs again) and starts nothing. The person starts or \
  dismisses it in the conversation. When auto mode is on for the project (get_auto_mode's new_runs), it starts the \
  run instead, through the same checks, and answers with the run. Refusals name why.";

/// What the agent reads first of `duplicate_pipeline`.
const DUPLICATE_AS_DRAFT: &str = "In this chat it saves the copy as a draft, and its first version is published as \
  publish_pipeline publishes in this chat: proposed to the person, unless they turned on Let the Orchestrator publish \
  drafts and what it widens is allowed. The answer names the copy and how its publication went.";

/// What the agent reads first of `cancel_run`.
const PROPOSE_CHANGE: &str = "In this chat it changes nothing itself: it checks what the call names and proposes the \
  change to the person, who applies or dismisses it in the conversation; once applied, it is done as follows.";

/// What the agent reads first of `merge_run`.
const PROPOSE_MERGE: &str = "In this chat it checks the run and proposes the merge to the person, who applies or \
  dismisses it in the conversation, unless the person turned on Let the Orchestrator merge runs in Settings: then it \
  merges at once. Call it only for a run the person asked you in this chat to merge. Either way the merge keeps the \
  rules below: only a task's latest run that succeeded, and only when the main checkout needs no local integration; \
  a refusal (needs_decision, merge_failed) leaves conflict choices and approving the merged result to the person on \
  the Local integration page.";

/// What the agent reads first of `answer_question` and `send_message`.
const PROPOSE_ANSWER: &str = "In this chat it checks the answer and proposes it to the person, who applies or \
  dismisses it in the conversation, unless the person turned on Let the Orchestrator answer agents in Settings: then \
  it is sent at once. Either way it is recorded as automation's and the task's conversation names you as its \
  sender. A message that would decide the plan a run waits on is refused (plan_decision_person_only): plan \
  decisions are the person's. It is done as follows.";

/// What the agent reads first of a tool that publishes a draft.
const PROPOSE_PUBLISH: &str = "In this chat it proposes the publication to the person, who applies or dismisses it \
  in the conversation, unless the person turned on Let the Orchestrator publish drafts in Settings: then it \
  publishes at once. Either way it is done as follows, and what widens what agents may do still needs Let \
  automation change agent permissions.";

/// Whose tools they are: the project's, for one conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatScope {
  pub project: ProjectId,
  /// The project's name, which a call may name it by.
  pub name: String,
  pub root: PathBuf,
  pub chat: ChatId,
}

/// Calls the chat's tool `name` with `arguments` in `scope`; a change it made, proposed or refused is kept in the
/// conversation, and `runners` tell the chat it changed.
pub async fn call(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  name: &str,
  mut arguments: JsonObject,
) -> Result<Value, ToolError> {
  let Some(tool) = listed().into_iter().find(|tool| tool.name == name) else {
    return Err(no_tool(name));
  };
  let read_only = tool.annotations.and_then(|hints| hints.read_only_hint) == Some(true);
  if read_only {
    own_project_by_id(scope, &mut arguments);
    return app_tool((services, translate), name, arguments).await;
  }
  in_own_project(scope, &mut arguments)?;
  if name == DUPLICATE_PIPELINE {
    return duplicate::duplicate((services, translate, runners), scope, arguments).await;
  }
  act_and_keep((services, translate, runners), scope, name, &arguments).await
}

/// Carries out or proposes the change `name` with `arguments`, given the chat's project, and keeps its action in the
/// conversation.
pub(super) async fn act_and_keep(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Result<Value, ToolError> {
  let acted = act((services, translate), scope, name, arguments).await;
  let Acted { result, kept } = acted;
  let outcome = match (&result, kept.waits) {
    (Ok(_), Some(waits)) => waits,
    (Ok(_), None) => ActionOutcome::Done,
    (Err(error), _) => keep::refusal(error, translate),
  };
  let named = kept.task.map(Named::Task).or_else(|| keep::named_task(arguments));
  let named = named.or_else(|| result.as_ref().ok().and_then(keep::created_task));
  // A hold that held new starts keeps what it held, for its card.
  let held = (name == HOLD_QUEUE && outcome == ActionOutcome::Done).then(|| keep::arguments_task(&named));
  let action = ChatAction {
    change: kept.change.map(Box::new),
    started: kept.started,
    auto_mode: kept.auto_mode,
    ..ChatAction::new(name, None, outcome)
  };
  keep::keep(scope, (named, held), action).await;
  runners.chat_changed(&scope.root);
  result
}

/// What a change did: what the agent is told, and what its action keeps.
struct Acted {
  result: Result<Value, ToolError>,
  kept: Kept,
}

impl Acted {
  fn done(result: Result<Value, ToolError>) -> Self {
    Self {
      result,
      kept: Kept::default(),
    }
  }
}

/// What an action keeps beyond the call: how it waits for the person when it does, the key of the task it names,
/// what it publishes, cancels or merges, the run it started, and whether it was done without asking because auto
/// mode is on.
#[derive(Default)]
struct Kept {
  waits: Option<ActionOutcome>,
  task: Option<String>,
  change: Option<ProposedChange>,
  started: Option<RunId>,
  auto_mode: bool,
}

/// Carries out the change `name` with `arguments` in `scope`, or proposes it to the person.
async fn act(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Acted {
  let acted = match name {
    RUN_TASK => start((services, translate), scope, arguments).await,
    CANCEL_RUN => propose_change(translate, scope, name, arguments).await,
    MERGE_RUN => switched((services, translate), scope, (name, arguments), OrchestratorConsent::MergeRuns).await,
    ANSWER_QUESTION | SEND_MESSAGE => {
      switched((services, translate), scope, (name, arguments), OrchestratorConsent::AnswerAgents).await
    }
    _ if Publishing::of_tool(name).is_some() => publish((services, translate), scope, name, arguments).await,
    _ => Ok(Acted::done(
      app_tool((services, translate), name, arguments.clone()).await,
    )),
  };
  acted.unwrap_or_else(|error| Acted::done(Err(error)))
}

/// A merge, an answer or a message: proposed to the person, unless the person's switch `consent` lets the chat do it;
/// then done at once, exactly as it was checked ([`change::apply`]: a merge merges only the commit checked), and kept
/// as the chat's own action.
async fn switched(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  (name, arguments): (&str, &JsonObject),
  consent: OrchestratorConsent,
) -> Result<Acted, ToolError> {
  if !allows(services, consent).await? {
    return propose_change(translate, scope, name, arguments).await;
  }
  let (change, task) = change::subject(scope, name, arguments).await?;
  let result = change::apply((services, translate), scope, &change, &[]).await;
  Ok(Acted {
    result,
    kept: Kept {
      task,
      change: Some(change),
      ..Kept::default()
    },
  })
}

/// `run_task`: a start proposed to the person, or, while auto mode is on for the project, the run started.
async fn start(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  arguments: &JsonObject,
) -> Result<Acted, ToolError> {
  if !auto_mode(scope).await? {
    let (told, proposal) = proposal::propose((services, translate), scope, arguments).await?;
    return Ok(Acted {
      result: Ok(told),
      kept: Kept {
        waits: Some(ActionOutcome::Proposed(Box::new(proposal.start))),
        task: Some(proposal.key),
        ..Kept::default()
      },
    });
  }
  let result = app_tool((services, translate), RUN_TASK, arguments.clone()).await;
  let result = match result {
    Err(error) => Err(blocked::named(scope, arguments.get("key").and_then(Value::as_str), error).await),
    told => told,
  };
  let started = result
    .as_ref()
    .ok()
    .and_then(|told| told.pointer("/run/id"))
    .and_then(Value::as_str)
    .and_then(|id| id.parse::<RunId>().ok());
  Ok(Acted {
    result,
    kept: Kept {
      started,
      auto_mode: true,
      ..Kept::default()
    },
  })
}

/// A cancel, merge or publication the person applies.
async fn propose_change(
  translate: &Translate,
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Result<Acted, ToolError> {
  let (change, task) = change::subject(scope, name, arguments).await?;
  Ok(Acted {
    result: Ok(change::told(translate, &change)),
    kept: Kept {
      waits: Some(ActionOutcome::Waits),
      task,
      change: Some(change),
      ..Kept::default()
    },
  })
}

/// A publication: proposed to the person, unless they let the Orchestrator publish drafts; then published, unless
/// it widens what agents may do while *Let automation change agent permissions* is off.
async fn publish(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Result<Acted, ToolError> {
  if !allows(services, OrchestratorConsent::PublishDrafts).await? {
    return propose_change(translate, scope, name, arguments).await;
  }
  let (change, _) = change::subject(scope, name, arguments).await?;
  if !change::widens(scope, &change).await?.is_empty() && !changes_permissions(services).await? {
    return propose_change(translate, scope, name, arguments).await;
  }
  // The tool is given the draft that was checked, by its id: a name could name another one by now.
  let (tool, arguments) = change::tool_arguments(scope, &change);
  let result = app_tool((services, translate), tool, arguments).await;
  Ok(Acted {
    result,
    kept: Kept {
      change: Some(change),
      ..Kept::default()
    },
  })
}

/// Whether auto mode is on for `scope`'s project: its new runs use it.
async fn auto_mode(scope: &ChatScope) -> Result<bool, ToolError> {
  let root = scope.root.clone();
  blocking(move || Ok(ProjectDb::open(&root)?.auto_mode_settings()?.new_runs)).await
}

/// Whether the person lets the Orchestrator chat do what `consent` names itself.
async fn allows(services: &Services, consent: OrchestratorConsent) -> Result<bool, ToolError> {
  let workspace = services.workspace().clone();
  blocking(move || Ok(workspace.orchestrator_allows(consent)?)).await
}

/// Whether the person lets automation widen what agents may do.
async fn changes_permissions(services: &Services) -> Result<bool, ToolError> {
  let workspace = services.workspace().clone();
  blocking(move || Ok(workspace.automation_changes_permissions()?)).await
}

/// Calls the app's tool `name`, of either of its lists, with `arguments`.
pub(super) async fn app_tool(
  (services, translate): (&Services, &Translate),
  name: &str,
  arguments: JsonObject,
) -> Result<Value, ToolError> {
  if let Some(called) = app_mcp::call(services, translate, name, arguments.clone()).await {
    return called;
  }
  app_mcp::setup::call(services, translate, name, arguments)
    .await
    .unwrap_or_else(|| Err(no_tool(name)))
}

fn no_tool(name: &str) -> ToolError {
  ToolError::InvalidArguments(format!("no tool is named {name}"))
}

/// Holds a change to the chat's project: one that names it, by its id or its name, or names none, is given it by its
/// id; one that names another project is refused.
fn in_own_project(scope: &ChatScope, arguments: &mut JsonObject) -> Result<(), ToolError> {
  let own = match arguments.get("project") {
    None => true,
    Some(Value::String(named)) => names_own_project(scope, named),
    Some(_) => false,
  };
  if !own {
    return Err(ToolError::refused(
      "other_project",
      vec![Message::new("orchestrator.tools.otherProject")],
    ));
  }
  arguments.insert("project".to_owned(), Value::String(scope.project.to_string()));
  Ok(())
}

/// A read that names the chat's project, by its id or its name, is given it by its id: the name may be another
/// project's by now. A read of another project, or of every project, stays as it is.
fn own_project_by_id(scope: &ChatScope, arguments: &mut JsonObject) {
  if let Some(Value::String(named)) = arguments.get("project")
    && names_own_project(scope, named)
  {
    arguments.insert("project".to_owned(), Value::String(scope.project.to_string()));
  }
}

/// Whether `named` names the chat's project, by its id or its name.
fn names_own_project(scope: &ChatScope, named: &str) -> bool {
  named == scope.project.to_string() || named.eq_ignore_ascii_case(&scope.name)
}
