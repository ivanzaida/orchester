//! The domain tools the Orchestrator's agent gets: a part of Orchester's own MCP tools ([`crate::app_mcp`]), for
//! the chat's project only. Each call goes through the same tool an MCP client of the app calls, so it is refused
//! or carried out exactly as for that client: what only a person decides stays refused (`person_only`), approvals
//! stay with people and runs start through the scheduling checks. A call that names another project is refused
//! before it reaches the tool, and the tool is given the chat's project by its id, never by a name. What the agent
//! changed, or tried to, is kept in its conversation as an action.

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

use super::proposal;
use crate::{
  app_mcp::{self, Services, ToolError, Translate},
  messages::Message,
  runner::Runners,
  tasks::hold,
};

/// The tools of the chat, by name: reading the project's work, task drafts, proposing a task's run to the person
/// ([`proposal`]), pausing and resuming a run, and holding and releasing new starts. Approvals, plans, questions,
/// project setup and what removes work are not among them.
pub const TOOLS: [&str; 12] = [
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
  "hold_queue",
  "release_queue",
];

/// The chat's tools as `tools/list` answers.
pub fn listed() -> Vec<Tool> {
  app_mcp::listed()
    .into_iter()
    .filter(|tool| TOOLS.contains(&tool.name.as_ref()))
    .map(|mut tool| {
      if tool.name == proposal::RUN_TASK {
        tool.description = Some(PROPOSE_RUN.into());
      }
      tool
    })
    .collect()
}

/// What the agent reads of `run_task`: here it proposes the start.
const PROPOSE_RUN: &str = "Proposes running a task to the person: checks that the task could run now (a draft is \
  queued and started, a queued task starts, an ended one runs again) and starts nothing. The person starts or \
  dismisses it in the conversation. Refusals name why.";

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
  mut arguments: JsonObject,
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
  // The tool finds a project by its id or its name; the name the call gave may be another project's by now.
  arguments.insert("project".to_owned(), Value::String(scope.project.to_string()));
  let read_only = tool.annotations.and_then(|hints| hints.read_only_hint) == Some(true);
  let mut task = named_task(&arguments);
  let (result, proposed) = if name == proposal::RUN_TASK {
    match proposal::propose((services, translate), scope, &arguments).await {
      Ok((told, proposal)) => {
        task = Some(Named::Task(proposal.key));
        (Ok(told), Some(proposal.start))
      }
      Err(error) => (Err(error), None),
    }
  } else {
    let called = app_mcp::call(services, translate, name, arguments)
      .await
      .unwrap_or_else(|| Err(ToolError::InvalidArguments(format!("no tool is named {name}"))));
    (called, None)
  };
  if !read_only {
    let named = task.or_else(|| result.as_ref().ok().and_then(created_task));
    let outcome = match (&result, proposed) {
      (Ok(_), Some(start)) => ActionOutcome::Proposed(start),
      (Ok(_), None) => ActionOutcome::Done,
      (Err(error), _) => refusal(error, translate),
    };
    // A hold that held new starts keeps what it held, for its card.
    let held = (name == HOLD_QUEUE && outcome == ActionOutcome::Done).then(|| arguments_task(&named));
    keep(scope, (name, named, held), outcome).await;
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
  text("key")
    .or_else(|| text("task"))
    .map(Named::Task)
    .or_else(|| text("run").map(Named::Run))
}

/// The key of the task a created task's result names.
fn created_task(result: &Value) -> Option<Named> {
  result
    .pointer("/task/key")
    .and_then(Value::as_str)
    .map(|key| Named::Task(key.to_owned()))
}

/// The refusal `error` as the conversation keeps it, in the words of `translate`.
pub(super) fn refusal(error: &ToolError, translate: &Translate) -> ActionOutcome {
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

/// The tool whose action keeps what it held.
const HOLD_QUEUE: &str = "hold_queue";

/// The task a hold named, which names its group; `None` for the whole project.
fn arguments_task(named: &Option<Named>) -> Option<String> {
  match named {
    Some(Named::Task(task)) => Some(task.clone()),
    Some(Named::Run(_)) | None => None,
  }
}

/// Keeps the action of tool `tool`, with the key of the task it names and, for a hold of new starts (`held`: the task
/// that names its group, if any), what it held, in the conversation; one that cannot be kept is logged.
async fn keep(
  scope: &ChatScope,
  (tool, named, held): (&str, Option<Named>, Option<Option<String>>),
  outcome: ActionOutcome,
) {
  let (root, chat, tool) = (scope.root.clone(), scope.chat, tool.to_owned());
  let kept = tokio::task::spawn_blocking(move || {
    let mut db = ProjectDb::open(&root)?;
    let task = named.and_then(|named| task_key(&db, named));
    let held = match held {
      Some(named) => {
        let group = match named {
          Some(named) => hold::group_of_named(&db, &named)?,
          None => None,
        };
        Some(hold::held_work(&db, group)?)
      }
      None => None,
    };
    let action = ChatAction {
      tool,
      task,
      outcome,
      held,
    };
    db.add_chat_action(chat, &action, Timestamp::now())
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
