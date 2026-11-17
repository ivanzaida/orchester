//! `wait_on_task`, a tool of the Orchestrator chat's own (ORC-766; `docs/design/orchestrator-watch.md`, screen 04
//! `OKtX2` and decision 6): a run that waits for a message or a decision waits on another task of its project, kept as
//! the relation `orchester_core::run::RunWaitsOn` with its row in the conversation (*KON-41 · #17 waits on KON-52*).
//! It changes no run, so it is done at once in any turn. What ends the wait is [`follow`]'s: the waited task's merge
//! into the run's base branch is an event of the waiting task in the conversation that leads it, as is the waited
//! task canceled or closed without that merge; the person answering the run on its own wait card ends it too, and so
//! does the person dropping it ([`dropping`], ORC-784). How a wait stands, as its row and the task page show it, is
//! [`view`]'s.

pub mod dropping;
pub(super) mod follow;
pub mod view;

use orchester_core::{
  orchestrator::{ActionOutcome, ChatAction, WaitsOnRow},
  project::TaskKey,
  run::{Run, RunWaitsOn, WaitReason, WaitingRun, WaitsOnError},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::{
  handler::server::common::schema_for_type,
  model::{JsonObject, Tool, ToolAnnotations},
  schemars::JsonSchema,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  keep::{self, Named},
  led_start::Named as Found,
  own_project::in_own_project,
  team_task,
  tools::ChatScope,
};
use crate::{
  app_mcp::{Missing, Services, ToolError, Translate, blocking, find_run},
  messages::Message,
  runner::Runners,
  tasks::recorded::task_of,
};

/// The tool's name.
pub const WAIT_ON_TASK: &str = "wait_on_task";

/// What the chat's agent reads of the tool.
const DESCRIPTION: &str = "Makes a run of this project that waits for a message or a decision wait on another task \
  of this project, such as one you created for the work it needs (run: its id or number; task: the waited task's key \
  or id). It changes no run, so it is done at once in any turn, and kept in the conversation as KON-41 · #17 waits on \
  KON-52. When the waited task's work is merged into the waiting run's base branch, Orchester starts a turn about it \
  here, in the conversation that leads the waiting task, while Let the Orchestrator answer agents is on: tell the \
  run's agents with send_message that the work is merged, and the run brings the base branch into its worktree when \
  it goes on. The waited task canceled, or closed without that merge, is told the same way and ends the wait; the \
  person answering the run on its own card ends it too. A run waits on one task at a time. Refused: the run's own \
  task (own_task), a task of another project (other_project), a run that does not wait for a message or a decision \
  (not_waiting_for_person: a tool call's approval is answered at once), a run that already waits on a task \
  (already_waits) and a run without a base branch to merge into (no_base_branch).";

/// The tool as the chat's `tools/list` answers.
pub fn tool() -> Tool {
  Tool::new(WAIT_ON_TASK, DESCRIPTION, schema_for_type::<WaitOnTask>())
    .with_annotations(ToolAnnotations::new().read_only(false).open_world(false))
}

/// What the tool takes besides `project`, which may name the chat's own project only ([`in_own_project`]).
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct WaitOnTask {
  /// The waiting run's id, or its number (`17` or `#17`).
  pub run: String,
  /// The task it waits on: its key (`KON-52`) or its id.
  pub task: String,
}

/// A wait kept: its row, and the waiting run's task by its key.
struct Kept {
  task: String,
  row: WaitsOnRow,
  base: String,
}

/// Makes the run `arguments` names wait on the task they name in `scope`, and keeps the action, done or refused, in
/// the conversation; `runners` tell the chat it changed.
pub(super) async fn call(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  mut arguments: JsonObject,
) -> Result<Value, ToolError> {
  let named_run = arguments.get("run").and_then(Value::as_str).map(str::to_owned);
  let kept = match in_own_project(scope, &mut arguments) {
    Ok(()) => {
      arguments.remove("project");
      keep_wait(services, scope, arguments).await
    }
    Err(refused) => Err(refused),
  };
  let (named, action) = match &kept {
    Ok(kept) => (
      Named::Task(kept.task.clone()),
      ChatAction {
        waits_on: Some(kept.row.clone()),
        ..ChatAction::new(WAIT_ON_TASK, None, ActionOutcome::Done)
      },
    ),
    Err(error) => (
      Named::Run(named_run.unwrap_or_default()),
      ChatAction::new(WAIT_ON_TASK, None, keep::refusal(error, translate)),
    ),
  };
  keep::keep(scope, (Some(named), None), action).await;
  runners.chat_changed(&scope.root);
  kept.map(|kept| {
    json!({
      "wait": {
        "run": { "id": kept.row.run.to_string(), "number": kept.row.number },
        "task": kept.task,
        "waits_on": kept.row.waited,
        "base_branch": kept.base,
      }
    })
  })
}

async fn keep_wait(services: &Services, scope: &ChatScope, arguments: JsonObject) -> Result<Kept, ToolError> {
  let asked: WaitOnTask =
    serde_json::from_value(Value::Object(arguments)).map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
  // A team task that never ran here, and that nothing watches, is waited on too: it is found as the start finds it.
  // One nothing answers to is refused below, in the order the wait is checked.
  let waited = match team_task::found(services, scope, &asked.task).await {
    Ok(task) => Some(task),
    Err(ToolError::NotFound { .. }) => None,
    Err(refused) => return Err(refused),
  };
  let scope = scope.clone();
  blocking(move || {
    let mut db = ProjectDb::open(&scope.root)?;
    keep_in(&mut db, &scope, (&asked, waited))
  })
  .await
}

/// Keeps the wait `asked` names in `db`, the project database of `scope`, on `waited`, the task it names when one
/// answers to it.
fn keep_in(
  db: &mut ProjectDb,
  scope: &ChatScope,
  (asked, waited): (&WaitOnTask, Option<Found>),
) -> Result<Kept, ToolError> {
  let run = find_run(db, &asked.run)?;
  let number = run.number().get();
  let task = task_of(db, run.task())?.ok_or_else(|| ToolError::not_found(Missing::Task, run.task().to_string()))?;
  let waited = waited.ok_or_else(|| missing_task(db, &asked.task))?;
  let refused = |error: WaitsOnError| refusal(db, &error, number);
  let waits_for = waits_for(&run).ok_or_else(|| refused(WaitsOnError::NotWaitingForPerson))?;
  let base = run.base_branch().cloned().ok_or_else(|| {
    ToolError::refused(
      "no_base_branch",
      vec![Message::new("orchestrator.waitsOn.noBaseBranch").with("run", number.to_string())],
    )
  })?;
  let waiting = WaitingRun {
    project: scope.project,
    run: run.id(),
    task: run.task(),
    base: base.clone(),
    waits_for,
  };
  // The waited task was found in the chat's own project.
  let wait = RunWaitsOn::new(waiting, waited.id, scope.project).map_err(refused)?;
  let kept = db.keep_run_wait(&wait, Timestamp::now());
  match kept {
    Ok(()) => {}
    Err(StoreError::RunWaitsOn(error)) => return Err(refusal(db, &error, number)),
    Err(error) => return Err(error.into()),
  }
  Ok(Kept {
    task: task.key().to_string(),
    row: WaitsOnRow {
      run: run.id(),
      number,
      waited: waited.key,
    },
    base: base.as_str().to_owned(),
  })
}

/// Why no task of `db`'s project answers to `named`: a key of another project's is refused as such.
fn missing_task(db: &ProjectDb, named: &str) -> ToolError {
  match named.to_uppercase().parse::<TaskKey>() {
    Ok(key) if key.project() != db.project().key() => refusal(db, &WaitsOnError::OtherProject, 0),
    _ => ToolError::not_found(Missing::Task, named),
  }
}

/// What `run` waits for that a task may take as long as: a message, else a decision on its plan; `None` when it waits
/// for neither (an approval of a tool call is answered at once).
fn waits_for(run: &Run) -> Option<WaitReason> {
  if run.status().is_terminal() {
    return None;
  }
  if !run.awaiting_answer().is_empty() {
    return Some(WaitReason::Message);
  }
  (!run.awaiting_plan().is_empty()).then_some(WaitReason::Plan)
}

/// Why run `#number` cannot wait on the task, with a stable code.
fn refusal(db: &ProjectDb, error: &WaitsOnError, number: u32) -> ToolError {
  let (code, message) = match *error {
    WaitsOnError::OwnTask => ("own_task", Message::new("orchestrator.waitsOn.ownTask")),
    WaitsOnError::OtherProject => ("other_project", Message::new("orchestrator.waitsOn.otherProject")),
    WaitsOnError::NotWaitingForPerson => (
      "not_waiting_for_person",
      Message::new("orchestrator.waitsOn.notWaiting").with("run", number.to_string()),
    ),
    WaitsOnError::AlreadyWaits(waited) => {
      // A task that cannot be read is named by its id.
      let key = task_of(db, waited)
        .ok()
        .flatten()
        .map_or_else(|| waited.to_string(), |task| task.key().to_string());
      (
        "already_waits",
        Message::new("orchestrator.waitsOn.alreadyWaits")
          .with("run", number.to_string())
          .with("task", key),
      )
    }
  };
  ToolError::refused(code, vec![message])
}
