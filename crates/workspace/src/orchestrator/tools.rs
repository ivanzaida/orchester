//! The domain tools the Orchestrator's agent gets: a part of Orchester's own MCP tools ([`crate::app_mcp`]), for
//! the chat's project only. Each call goes through the same tool an MCP client of the app calls, so it is refused
//! or carried out exactly as for that client: what only a person decides stays refused (`person_only`), approvals
//! stay with people and runs start through the scheduling checks. A call that names another project is refused
//! before it reaches the tool. What the agent changed, or tried to, is kept in its conversation as an action.

use std::path::PathBuf;

use orchester_core::{
  id::{ChatId, ProjectId, RunId},
  orchestrator::{ActionOutcome, ChatAction},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::model::{JsonObject, Tool};
use serde_json::Value;

use crate::{
  app_mcp::{self, Services, ToolError, Translate},
  messages::Message,
  runner::Runners,
};

/// The tools of the chat, by name: reading the project's work, task drafts, starting a task's run, and pausing and
/// resuming a run. Approvals, plans, questions, project setup and what removes work are not among them.
pub const TOOLS: [&str; 10] = [
  "list_tasks",
  "get_task",
  "get_run",
  "list_approvals",
  "list_pipelines",
  "create_task",
  "update_task",
  "run_task",
  "pause_run",
  "resume_run",
];

/// The chat's tools as `tools/list` answers.
pub fn listed() -> Vec<Tool> {
  app_mcp::listed()
    .into_iter()
    .filter(|tool| TOOLS.contains(&tool.name.as_ref()))
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
}

/// Calls the chat's tool `name` with `arguments` in `scope`; a change it made or refused is kept in the
/// conversation, and `runners` tell the chat it changed.
pub async fn call(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  name: &str,
  arguments: JsonObject,
) -> Result<Value, ToolError> {
  let Some(tool) = listed().into_iter().find(|tool| tool.name == name) else {
    return Err(ToolError::InvalidArguments(format!("no tool is named {name}")));
  };
  if !names_own_project(scope, &arguments) {
    return Err(ToolError::refused(
      "other_project",
      vec![Message::new("orchestrator.tools.otherProject")],
    ));
  }
  let read_only = tool.annotations.and_then(|hints| hints.read_only_hint) == Some(true);
  let task = named_task(&arguments);
  let result = app_mcp::call(services, translate, name, arguments)
    .await
    .unwrap_or_else(|| Err(ToolError::InvalidArguments(format!("no tool is named {name}"))));
  if !read_only {
    let named = task.or_else(|| result.as_ref().ok().and_then(created_task));
    let outcome = match &result {
      Ok(_) => ActionOutcome::Done,
      Err(error) => refusal(error, translate),
    };
    keep(scope, name, named, outcome).await;
    runners.chat_changed(&scope.root);
  }
  result
}

/// Whether `arguments` name the chat's project, by its id or its name.
fn names_own_project(scope: &ChatScope, arguments: &JsonObject) -> bool {
  arguments
    .get("project")
    .and_then(Value::as_str)
    .is_some_and(|named| named == scope.project.to_string() || named.eq_ignore_ascii_case(&scope.name))
}

/// Which task the call names: its key, or the run it names, read when the action is kept.
enum Named {
  Task(String),
  Run(String),
}

fn named_task(arguments: &JsonObject) -> Option<Named> {
  let text = |name: &str| arguments.get(name).and_then(Value::as_str).map(str::to_owned);
  text("key").map(Named::Task).or_else(|| text("run").map(Named::Run))
}

/// The key of the task a created task's result names.
fn created_task(result: &Value) -> Option<Named> {
  result
    .pointer("/task/key")
    .and_then(Value::as_str)
    .map(|key| Named::Task(key.to_owned()))
}

fn refusal(error: &ToolError, translate: &Translate) -> ActionOutcome {
  let refused = error.to_json(|message| translate.text(message));
  let text = |name: &str| {
    refused
      .pointer(&format!("/error/{name}"))
      .and_then(Value::as_str)
      .unwrap_or_default()
      .to_owned()
  };
  ActionOutcome::Refused {
    code: text("code"),
    reason: text("message"),
  }
}

/// Keeps the action of tool `tool`, with the key of the task it names, in the conversation; one that cannot be kept
/// is logged.
async fn keep(scope: &ChatScope, tool: &str, named: Option<Named>, outcome: ActionOutcome) {
  let (root, chat, tool) = (scope.root.clone(), scope.chat, tool.to_owned());
  let kept = tokio::task::spawn_blocking(move || {
    let mut db = ProjectDb::open(&root)?;
    let task = named.and_then(|named| task_key(&db, named));
    db.add_chat_action(chat, &ChatAction { tool, task, outcome }, Timestamp::now())
  })
  .await;
  match kept {
    Ok(Ok(())) => {}
    Ok(Err(error)) => tracing::error!(%chat, %error, "cannot keep an action of the Orchestrator"),
    Err(error) => tracing::error!(%chat, %error, "keeping an action of the Orchestrator did not finish"),
  }
}

/// The key of the task `named` names, as people read it; `None` when it cannot be found.
fn task_key(db: &ProjectDb, named: Named) -> Option<String> {
  match named {
    Named::Task(key) => Some(key),
    Named::Run(run) => {
      let run = db.run(run.parse::<RunId>().ok()?).ok()??;
      Some(db.task(run.task()).ok()??.key().to_string())
    }
  }
}
