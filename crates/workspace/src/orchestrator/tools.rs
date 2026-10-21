//! The domain tools the Orchestrator's agent gets (`docs/design/orchestrator-toolset.md`): a part of Orchester's own
//! MCP tools ([`crate::app_mcp`]). Each call goes through the same tool an MCP client of the app calls, so it is
//! refused or carried out exactly as for that client: what only a person decides stays refused (`person_only`),
//! approvals and plan decisions stay with people and runs start through the scheduling checks. It reads every
//! project; a change that names another project is refused before it reaches the tool, and a change is given the
//! chat's project by its id, never by a name. Publishing a draft, canceling, merging a run, answering an agent's
//! question, sending a run's agents a message and deciding how a stopped run goes on wait for the person
//! ([`super::change`]) unless the person's switch lets the chat do it itself (publishing, merging, answering; never
//! canceling or deciding how a stopped run goes on), and so does starting a run unless auto mode is on for the project
//! ([`super::proposal`]). An interrupted run is continued only under the person's switch ([`super::guard`]), and a
//! finished run is merged in a turn nobody asked for only under the person's switches ([`super::auto_merge`]). The
//! chat's own `init_repository` is always the person's to apply ([`super::repository`]). What the agent changed, or
//! tried to, is kept in its conversation as an action; what it reads of each tool that works otherwise here is
//! [`super::told`]'s.

use std::path::PathBuf;

use orchester_core::{
  id::{ChatId, ProjectId, RunId},
  orchestrator::{
    ActionOutcome, ChatAction,
    change::{ProposedChange, Publishing},
  },
  run::Answerer,
};
use orchester_store::ProjectDb;
use rmcp::model::{JsonObject, Tool};
use serde_json::Value;

use super::{
  auto_merge::{self, MergeAs},
  blocked,
  change::{
    self,
    control::{self, QueueCall, RELEASE_QUEUE, RESUME_RUN, ToldTo},
  },
  duplicate, guard,
  keep::{self, HOLD_QUEUE, Named},
  own_project::{in_own_project, own_project_by_id},
  proposal,
  repository::{self, INIT_REPOSITORY},
  told,
};
use crate::{
  app_mcp::{self, Services, ToolError, Translate, blocking},
  runner::Runners,
  workspace::OrchestratorConsent,
};

/// The tools of the chat, by name: reading every project's work, setup and tasks' conversations, task drafts and
/// drafts of what agents run on, making and changing modules (drafts-level: they only organize tasks), pausing and
/// resuming a run, holding and releasing new starts, and what waits for the person or the person's switch: starting
/// a run, publishing a draft, canceling and merging a run, answering an agent's question, sending a run's agents a
/// message and continuing a run that waits for a person's decision. Approvals, plan decisions, project setup,
/// connections, MCP servers, the environment and what removes work are not among them. A name whose tool the app
/// does not have (yet) is not listed ([`listed`]).
pub const TOOLS: [&str; 59] = [
  "list_projects",
  "get_project",
  "list_tasks",
  "get_task",
  "create_task",
  "update_task",
  "list_modules",
  "get_module",
  "create_module",
  "update_module",
  "list_runs",
  "get_run",
  "get_changes",
  "get_conversation",
  "run_task",
  "pause_run",
  "resume_run",
  "cancel_run",
  "merge_run",
  "continue_run",
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
  "get_story_point_calibration",
  "get_daily_report",
  "list_pull_requests",
  "get_pull_request",
];

/// The chat's own tools, which no client of the app has: making the project's folder a git repository, always a card
/// the person applies ([`super::repository`]).
pub const CHAT_TOOLS: [&str; 1] = [INIT_REPOSITORY];

/// The tool whose call starts a run, or proposes it.
pub(super) const RUN_TASK: &str = "run_task";
/// The tool whose calls always wait for the person.
pub(super) const CANCEL_RUN: &str = "cancel_run";
/// The tool whose calls wait for the person unless *Let the Orchestrator merge runs* is on.
pub(in crate::orchestrator) const MERGE_RUN: &str = "merge_run";
/// The tool whose calls always wait for the person, and only for a retry, a return to an earlier step or an agent's
/// resolution of a merge's conflicts.
pub(super) const CONTINUE_RUN: &str = "continue_run";
/// The tools whose calls wait for the person unless *Let the Orchestrator answer agents* is on.
pub(super) const ANSWER_QUESTION: &str = "answer_question";
pub(super) const SEND_MESSAGE: &str = "send_message";
/// The tool whose copy the chat saves as a draft before it publishes it by the publication rule.
pub(super) const DUPLICATE_PIPELINE: &str = "duplicate_pipeline";

/// The chat's tools as `tools/list` answers: those of [`TOOLS`] the app has, each told how it works in the chat, and
/// the chat's own ([`CHAT_TOOLS`]).
pub fn listed() -> Vec<Tool> {
  app_mcp::listed()
    .into_iter()
    .chain(app_mcp::setup::listed())
    .filter(|tool| TOOLS.contains(&tool.name.as_ref()))
    .map(|mut tool| {
      if let Some(told) = told::in_chat(&tool) {
        tool.description = Some(told.into());
      }
      tool
    })
    .chain([repository::tool()])
    .collect()
}

/// Whose tools they are: the project's, for one conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatScope {
  pub project: ProjectId,
  /// The project's name, which a call may name it by.
  pub name: String,
  pub root: PathBuf,
  pub chat: ChatId,
  /// A wait, a run's end or a day's report started the turn by itself: nobody asked for anything in it; which one
  /// started it, the guard reads from the conversation ([`super::guard`]).
  pub on_its_own: bool,
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
  // A copy saves a draft: a turn a day's report started refuses it, and keeps the refusal.
  if name == DUPLICATE_PIPELINE && !guard::in_report_turn(scope).await? {
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
    resumed: kept.resumed,
    auto_mode: kept.auto_mode,
    on_its_own: kept.on_its_own,
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
/// mode is on, or merged on its own because the person lets the Orchestrator merge finished runs.
#[derive(Default)]
struct Kept {
  waits: Option<ActionOutcome>,
  task: Option<String>,
  change: Option<ProposedChange>,
  started: Option<RunId>,
  resumed: Option<RunId>,
  auto_mode: bool,
  on_its_own: bool,
}

/// Carries out the change `name` with `arguments` in `scope`, or proposes it to the person.
async fn act(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Acted {
  // In a turn nobody asked for, what would change how runs go on or what agents run on is only proposed; in one a
  // day's report started, every change is, or refused.
  let proposes = match guard::only_proposed(scope, name).await {
    Ok(proposes) => proposes,
    Err(error) => return Acted::done(Err(error)),
  };
  let acted = match name {
    // Only the turn of a run's success merges on its own; a report's turn finds no such wait and proposes.
    MERGE_RUN if scope.on_its_own => merge_on_its_own((services, translate), scope, arguments).await,
    _ if proposes => propose_change((services, translate), scope, name, arguments).await,
    RUN_TASK => start((services, translate), scope, arguments).await,
    // Making a repository writes to the project's folder: always the person's, whatever auto mode and the switches.
    CANCEL_RUN | CONTINUE_RUN | INIT_REPOSITORY => propose_change((services, translate), scope, name, arguments).await,
    MERGE_RUN => {
      switched(
        (services, translate),
        scope,
        (name, arguments),
        OrchestratorConsent::MergeRuns,
      )
      .await
    }
    RESUME_RUN => resume((services, translate), scope, arguments).await,
    ANSWER_QUESTION | SEND_MESSAGE => {
      switched(
        (services, translate),
        scope,
        (name, arguments),
        OrchestratorConsent::AnswerAgents,
      )
      .await
    }
    _ if Publishing::of_tool(name).is_some() => publish((services, translate), scope, name, arguments).await,
    // A hold or a release that changed nothing is refused, so its card shows no change that did not happen.
    HOLD_QUEUE | RELEASE_QUEUE => {
      let call = if name == HOLD_QUEUE {
        QueueCall::Hold
      } else {
        QueueCall::Release(ToldTo::Agent)
      };
      let task = arguments.get("task").and_then(Value::as_str);
      let told = match app_tool((services, translate), name, arguments.clone()).await {
        Ok(told) => control::queue_changed(told, call, (scope, task)).await,
        refused => refused,
      };
      Ok(Acted::done(told))
    }
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
    return propose_change((services, translate), scope, name, arguments).await;
  }
  let (change, task) = change::subject((scope, services), name, arguments).await?;
  // A message that moves a run none of whose agents waits or works is the person's to send.
  if change.change.moves_run() {
    return Ok(proposed(translate, change, task));
  }
  let result = change::apply((services, translate), scope, &change, &[], Answerer::Automation).await;
  Ok(Acted {
    result,
    kept: Kept {
      task,
      change: Some(change),
      ..Kept::default()
    },
  })
}

/// `merge_run` in a turn nobody asked for: proposed, unless the person lets the Orchestrator merge finished runs on its
/// own (with *merge runs* and *answer agents*) and the turn is the one the run's success started to merge it
/// ([`auto_merge::merge_as`]); then merged at once, exactly as it was checked, and kept as the chat's own action on
/// its own. A `merge_failed` names the next step as any merge of the chat's does ([`change::apply`]).
async fn merge_on_its_own(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  arguments: &JsonObject,
) -> Result<Acted, ToolError> {
  let switched_on = allows(services, OrchestratorConsent::MergeFinished).await?
    && allows(services, OrchestratorConsent::MergeRuns).await?
    && allows(services, OrchestratorConsent::AnswerAgents).await?;
  if auto_merge::merge_as(scope, arguments, switched_on).await? == MergeAs::Propose {
    return propose_change((services, translate), scope, MERGE_RUN, arguments).await;
  }
  let (change, task) = change::subject((scope, services), MERGE_RUN, arguments).await?;
  let result = change::apply((services, translate), scope, &change, &[], Answerer::Automation).await;
  Ok(Acted {
    result,
    kept: Kept {
      task,
      change: Some(change),
      on_its_own: true,
      ..Kept::default()
    },
  })
}

/// `run_task`: a start proposed to the person, or, while auto mode is on for the project, the run started; refused for
/// a task marked Blocked.
async fn start(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  arguments: &JsonObject,
) -> Result<Acted, ToolError> {
  blocked::marked(scope, arguments.get("key").and_then(Value::as_str)).await?;
  // In a turn nobody asked for, a start is only proposed.
  if scope.on_its_own || !auto_mode(scope).await? {
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

/// `resume_run`: proposed, done or refused as [`guard::resume_as`] tells; done, it resumes the run or continues an
/// interrupted one from its last checkpoint through the same tool as the task page's controls, and the run is kept.
async fn resume(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  arguments: &JsonObject,
) -> Result<Acted, ToolError> {
  let switches = guard::ResumeSwitches {
    continues: allows(services, OrchestratorConsent::ContinueInterrupted).await?,
    answers: allows(services, OrchestratorConsent::AnswerAgents).await?,
  };
  let run = match guard::resume_as(scope, arguments, switches).await? {
    guard::Resume::Propose => return propose_change((services, translate), scope, RESUME_RUN, arguments).await,
    guard::Resume::Now(run) => run,
  };
  let result = app_tool((services, translate), RESUME_RUN, arguments.clone()).await;
  let resumed = result.is_ok().then_some(run);
  Ok(Acted {
    result,
    kept: Kept {
      resumed,
      ..Kept::default()
    },
  })
}

/// A cancel, merge, publication, answer, message or continuation the person applies.
async fn propose_change(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  name: &str,
  arguments: &JsonObject,
) -> Result<Acted, ToolError> {
  let (change, task) = change::subject((scope, services), name, arguments).await?;
  Ok(proposed(translate, change, task))
}

/// `change`, of task `task`, proposed to the person: what the agent is told, and what its action keeps.
fn proposed(translate: &Translate, change: ProposedChange, task: Option<String>) -> Acted {
  Acted {
    result: Ok(change::told(translate, &change)),
    kept: Kept {
      waits: Some(ActionOutcome::Waits),
      task,
      change: Some(change),
      ..Kept::default()
    },
  }
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
    return propose_change((services, translate), scope, name, arguments).await;
  }
  let (change, _) = change::subject((scope, services), name, arguments).await?;
  if !change::widens(scope, &change).await?.is_empty() && !changes_permissions(services).await? {
    return propose_change((services, translate), scope, name, arguments).await;
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
