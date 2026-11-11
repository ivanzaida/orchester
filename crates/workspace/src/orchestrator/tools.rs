//! The domain tools the Orchestrator's agent gets (`docs/design/orchestrator-toolset.md`): a part of Orchester's own
//! MCP tools ([`crate::app_mcp`]). Each call goes through the same tool an MCP client of the app calls, so it is
//! refused or carried out exactly as for that client: what only a person decides stays refused (`person_only`) and
//! runs start through the scheduling checks. It reads every project; a change that names another project is refused
//! before it reaches the tool, and a change is given the chat's project by its id, never by a name. While the
//! project's switch *Let the Orchestrator drive* is on, the chat has every domain tool of the app and each change is
//! carried out at once; while it is off, publishing a draft, canceling, merging a run, answering an agent's question,
//! sending a run's agents a message and deciding how a stopped run goes on wait for the person ([`super::change`]),
//! and so does starting a run unless the project allows Autopilot ([`super::proposal`]) ([`super::acts`]). The chat's
//! own `init_repository` makes the project a git repository ([`super::repository`]); its git tools read always and
//! write while the chat drives, and only propose what can lose work ([`super::git`]); its MCP server tools only
//! propose what widens what agents may do while it does not drive ([`super::mcp_servers`]). What the agent changed, or
//! tried to, is kept in its conversation as an action; what it reads of each tool that works otherwise here is
//! [`super::told`]'s.

use std::path::PathBuf;

use orchester_core::id::{ChatId, ProjectId};
use rmcp::model::{JsonObject, Tool};
use serde_json::Value;

use super::{
  acts::{act_and_keep, drives},
  approval::{self, APPLY_PROPOSAL},
  approval_turn, duplicate, git, guard, leads, mcp_servers,
  own_project::{in_own_project, own_project_by_id},
  plan::{self, PROPOSE_PLAN},
  repository::{self, INIT_REPOSITORY},
  told,
  waits_on::{self, WAIT_ON_TASK},
  watch::tool as watch_tool,
};
use crate::{
  app_mcp::{self, Services, ToolError, Translate, blocking},
  runner::Runners,
};

/// The tools of the chat while it does not drive, by name: reading every project's work, setup and tasks'
/// conversations, task drafts and drafts of what agents run on, making and changing modules (drafts-level: they only
/// organize tasks), pausing and resuming a run, holding and releasing new starts, and what waits for the person:
/// starting a run, publishing a draft, canceling and merging a run, answering an agent's question, sending a run's
/// agents a message and continuing a run that waits for a person's decision. While it drives, it has every other tool
/// of the app too ([`listed`]). A name whose tool the app does not have (yet) is not listed.
pub const TOOLS: [&str; 58] = [
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
  "get_approvals",
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
  "get_delivery",
  "get_story_point_calibration",
  "get_daily_report",
];

/// The chat's own tools, which no client of the app has: making the project's folder a git repository, always a card
/// the person applies ([`super::repository`]), working with git in it ([`super::git`]), changing the project's
/// MCP servers, a card the person applies for whatever widens ([`super::mcp_servers`]), applying a proposal the
/// person approved in their own words ([`super::approval`]), answering with a plan in plan-only mode
/// ([`super::plan`]), making a waiting run wait on another task ([`super::waits_on`]), and watching and stopping
/// watching at the person's request in words ([`watch_tool`]).
pub const CHAT_TOOLS: [&str; 27] = [
  watch_tool::WATCH,
  watch_tool::UNWATCH,
  INIT_REPOSITORY,
  APPLY_PROPOSAL,
  PROPOSE_PLAN,
  WAIT_ON_TASK,
  git::NAMES[0],
  git::NAMES[1],
  git::NAMES[2],
  git::NAMES[3],
  git::NAMES[4],
  git::NAMES[5],
  git::NAMES[6],
  git::NAMES[7],
  git::NAMES[8],
  git::NAMES[9],
  git::NAMES[10],
  git::NAMES[11],
  git::NAMES[12],
  git::NAMES[13],
  git::NAMES[14],
  git::NAMES[15],
  git::NAMES[16],
  mcp_servers::NAMES[0],
  mcp_servers::NAMES[1],
  mcp_servers::NAMES[2],
  mcp_servers::NAMES[3],
];

/// The tool whose call starts a run, or proposes it.
pub(super) const RUN_TASK: &str = "run_task";
/// The tool whose calls wait for the person unless the chat drives.
pub(super) const CANCEL_RUN: &str = "cancel_run";
pub(in crate::orchestrator) const MERGE_RUN: &str = "merge_run";
/// The tool whose calls wait for the person unless the chat drives; while it does not, only for a retry, a return to
/// an earlier step or an agent's resolution of a merge's conflicts.
pub(super) const CONTINUE_RUN: &str = "continue_run";
pub(super) const ANSWER_QUESTION: &str = "answer_question";
pub(super) const SEND_MESSAGE: &str = "send_message";
/// The tool whose copy the chat saves as a draft before it publishes it by the publication rule.
pub(super) const DUPLICATE_PIPELINE: &str = "duplicate_pipeline";

/// The app's tools the chat does not get even while it drives: creating a project is no work of one project's chat,
/// and the chat has MCP server tools of its own ([`mcp_servers::NAMES`]) that apply the app's.
const NOT_DRIVEN: [&str; 5] = [
  "add_project",
  "add_mcp_server",
  "update_mcp_server",
  "set_mcp_server_enabled",
  "remove_mcp_server",
];

/// The chat's tools as `tools/list` answers: those of [`TOOLS`] the app has, and, while the chat `drives`, every other
/// domain tool of the app but [`NOT_DRIVEN`]; each told how it works in the chat, and the chat's own ([`CHAT_TOOLS`]).
pub fn listed(drives: bool) -> Vec<Tool> {
  app_mcp::listed()
    .into_iter()
    .chain(app_mcp::setup::listed())
    .filter(|tool| {
      let name = tool.name.as_ref();
      TOOLS.contains(&name) || (drives && !NOT_DRIVEN.contains(&name))
    })
    .map(|mut tool| {
      if let Some(told) = told::in_chat(&tool, drives) {
        tool.description = Some(told.into());
      }
      approval_turn::with_reason(&mut tool);
      tool
    })
    .chain([repository::tool(), approval::tool(), plan::tool(), waits_on::tool()])
    .chain(git::tools())
    .chain(mcp_servers::tools())
    .chain(watch_tool::tools())
    .collect()
}

/// The chat's tools in `scope` as `tools/list` answers: [`listed`] for whether the project lets the chat drive.
pub async fn listed_in(services: &Services, scope: &ChatScope) -> Result<Vec<Tool>, ToolError> {
  Ok(listed(drives(services, scope).await?))
}

/// Whether the chat's tool `name` only reads, as its listing says; one the chat does not have changes something.
fn reads_only(tools: &[Tool], name: &str) -> bool {
  tools
    .iter()
    .find(|tool| tool.name == name)
    .and_then(|tool| tool.annotations.as_ref())
    .and_then(|hints| hints.read_only_hint)
    == Some(true)
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
/// conversation (a proposal it applied for the person, on its card), and `runners` tell the chat it changed. In a turn
/// that plans, every tool that changes something is refused before it is reached, and the refusal kept
/// ([`super::plan`]). The tools it has depend on whether the project lets it drive ([`listed`]).
pub async fn call(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  name: &str,
  mut arguments: JsonObject,
) -> Result<Value, ToolError> {
  let tools = listed(drives(services, scope).await?);
  if !tools.iter().any(|tool| tool.name == name) {
    return Err(no_tool(name));
  }
  if name == PROPOSE_PLAN {
    return plan::call((translate, runners), scope, arguments).await;
  }
  let read_only = reads_only(&tools, name);
  if !read_only && plan::planning(scope).await? {
    return Err(plan::refuse((translate, runners), scope, name).await);
  }
  if let Some(kind) = git::kind(name) {
    return git::chat::call((services, translate, runners), scope, (name, kind), arguments).await;
  }
  if mcp_servers::is_tool(name) {
    return mcp_servers::call((services, translate, runners), scope, name, arguments).await;
  }
  if watch_tool::is_tool(name) {
    return watch_tool::call(runners, scope, name, arguments).await;
  }
  if name == APPLY_PROPOSAL {
    return approval::call((services, translate, runners), scope, arguments).await;
  }
  if name == WAIT_ON_TASK {
    return waits_on::call((translate, runners), scope, arguments).await;
  }
  if read_only {
    own_project_by_id(scope, &mut arguments);
    return leads::read_tool((services, translate), scope, name, arguments).await;
  }
  in_own_project(scope, &mut arguments)?;
  // A copy saves a draft: a turn a day's report started refuses it, and keeps the refusal.
  if name == DUPLICATE_PIPELINE && !guard::in_report_turn(scope).await? {
    return duplicate::duplicate((services, translate, runners), scope, arguments).await;
  }
  act_and_keep((services, translate, runners), scope, name, &arguments).await
}

/// Whether the person lets automation widen what agents may do.
pub(super) async fn changes_permissions(services: &Services) -> Result<bool, ToolError> {
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
