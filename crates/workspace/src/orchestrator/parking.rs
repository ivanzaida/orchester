//! Parking a task for the person (`orchester_core::orchestrator::parking`,
//! `docs/plan/orchestrator-autonomy/OA-15-…`). The chat's tool `park_task` leaves a task it cannot move to the person
//! with what exactly they are to do: the task then reads as parked where its target stands ([`super::standing`]),
//! nothing starts it, the events of its run are tried no more, and in a turn nobody asked for the parking is a
//! decision the person reviews ([`super::decisions`]). It ends when the task's run moves or it gets another (someone
//! acted on it: [`end_moved`]), or when the person gives it back ([`give_back_in`]). The same in a local and a team
//! project: the record is this computer's, where the Orchestrator runs.

use std::path::Path;

use orchester_core::{
  id::TaskId,
  orchestrator::{
    ActionOutcome, ChatAction,
    parking::{MAX_WORDS, Needs, Parking, Words},
  },
  store::RunStore,
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
  decisions, guard,
  keep::{self, Named},
  own_project::in_own_project,
  team_status::module_now,
  tools::ChatScope,
};
use crate::{
  app_mcp::{ToolError, Translate, blocking, find_task},
  messages::{self, Message},
  runner::Runners,
  tasks::detail::run_store_error,
};

/// The tool's name.
pub const PARK_TASK: &str = "park_task";

/// What the chat's agent reads of the tool.
const DESCRIPTION: &str = "Parks a task of this project for the person when you cannot move it yourself: a decision   that is theirs (irreversible, outward, or one the brief does not settle), a request of an agent only they may   allow, a sign-in or credentials, a tool that does not answer after you tried again, or a ceiling of your leading   that was reached. Say what kind of thing is needed (needs), why you could not go on (why) and what exactly they   are to do (to_do), each in a sentence or two of at most 600 characters. The task then reads as parked where its   target stands (get_standing), nothing starts it, the events of its run start no turn again, and the tasks that   wait on it read as blocked by a parked task; go on with the rest. It ends by itself when the person acts on the   task's run or gives the task back. Do not park to avoid work you can do: in a turn nobody asked for, a parking is   kept as a decision of yours for the person to review, and only a task your conversation leads is parked   (not_led). Refused in a turn a day's report or a subscription's end started (report_turn).";

/// The tool as the chat's `tools/list` answers.
pub fn tool() -> Tool {
  Tool::new(PARK_TASK, DESCRIPTION, schema_for_type::<ParkTask>())
    .with_annotations(ToolAnnotations::new().read_only(false).open_world(false))
}

/// What kind of thing the person is asked for.
#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum NeedsArg {
  Decision,
  Approval,
  SignIn,
  Tool,
  Ceiling,
  Other,
}

impl From<NeedsArg> for Needs {
  fn from(needs: NeedsArg) -> Self {
    match needs {
      NeedsArg::Decision => Self::Decision,
      NeedsArg::Approval => Self::Approval,
      NeedsArg::SignIn => Self::SignIn,
      NeedsArg::Tool => Self::Tool,
      NeedsArg::Ceiling => Self::Ceiling,
      NeedsArg::Other => Self::Other,
    }
  }
}

/// What the tool takes besides `project`, which may name the chat's own project only ([`in_own_project`]).
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ParkTask {
  /// The task: its key (`KON-52`) or its id.
  pub task: String,
  /// What kind of thing the person is asked for.
  pub needs: NeedsArg,
  /// Why you could not go on with it.
  pub why: String,
  /// What exactly the person is to do.
  pub to_do: String,
}

/// Parks the task `arguments` name in `scope`, and keeps the action, done or refused, in the conversation; in a turn
/// nobody asked for, the parking is kept as a decision too. `runners` tell the chat it changed.
pub(super) async fn call(
  (translate, runners): (&Translate, &Runners),
  scope: &ChatScope,
  mut arguments: JsonObject,
) -> Result<Value, ToolError> {
  let named = arguments
    .get("task")
    .and_then(Value::as_str)
    .unwrap_or_default()
    .to_owned();
  let why = arguments
    .get("why")
    .and_then(Value::as_str)
    .unwrap_or_default()
    .to_owned();
  let parked = match in_own_project(scope, &mut arguments) {
    Ok(()) => {
      arguments.remove("project");
      park(scope, arguments.clone()).await
    }
    Err(refused) => Err(refused),
  };
  let outcome = match &parked {
    Ok(_) => ActionOutcome::Done,
    Err(error) => keep::refusal(error, translate),
  };
  let action = ChatAction::new(PARK_TASK, None, outcome);
  keep::keep(scope, (Some(Named::Task(named)), None), action).await;
  if parked.is_ok() && scope.on_its_own {
    decisions::keep(scope, PARK_TASK, &arguments, (None, &why)).await;
  }
  runners.chat_changed(&scope.root);
  parked
}

async fn park(scope: &ChatScope, arguments: JsonObject) -> Result<Value, ToolError> {
  if guard::in_report_turn(scope).await? {
    return Err(ToolError::refused(
      "report_turn",
      vec![Message::new("orchestrator.tools.reportChangesNothing")],
    ));
  }
  let asked: ParkTask =
    serde_json::from_value(Value::Object(arguments)).map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
  let words = |name: &str, text: &str| {
    Words::new(text).ok_or_else(|| {
      ToolError::InvalidArguments(format!("{name}: a sentence or two of at most {MAX_WORDS} characters"))
    })
  };
  let (why, to_do) = (words("why", &asked.why)?, words("to_do", &asked.to_do)?);
  let scope = scope.clone();
  blocking(move || {
    let mut db = ProjectDb::open(&scope.root)?;
    let task = find_task(&db, &asked.task)?;
    let key = task.key().to_string();
    let leads = db.active_subscriptions()?.leading(task.id(), module_now(&db, &task)) == Some(scope.chat);
    if scope.on_its_own && !leads {
      return Err(ToolError::refused(
        "not_led",
        vec![Message::new("orchestrator.start.notLed").with("task", key)],
      ));
    }
    let latest = db.runs_of_task(task.id()).map_err(run_store_error)?.pop();
    let run = latest.and_then(Result::ok);
    let parking = Parking {
      task: task.id(),
      run: run.as_ref().map(|run| run.id()),
      needs: asked.needs.into(),
      why,
      to_do,
      by: scope.chat,
      at: Timestamp::now(),
    };
    db.park_task(&parking)?;
    if let Some(run) = &run {
      db.settle_waits_of_run(run.id())?;
    }
    Ok(json!({
      "parked": {
        "task": key,
        "needs": parking.needs.name(),
        "run": run.map(|run| run.number().get()),
      }
    }))
  })
  .await
}

/// Ends the parkings of the tasks someone acted on since they were parked: the task's run moved, or it has another
/// one. Whether it ended any. Blocks on SQLite.
pub(super) fn end_moved(db: &mut ProjectDb) -> Result<bool, StoreError> {
  let moved = db.parkings_moved()?;
  for task in &moved {
    db.unpark_task(*task)?;
  }
  Ok(!moved.is_empty())
}

/// The person gives parked task `task` of the project at `root` back to the Orchestrator; whether it was parked.
/// Refused, with the parking kept, while a tool request of the run it was parked with still waits: only a person
/// answers one, so the chat would park the task again at once. What else a task was parked for (a decision, a
/// sign-in, a tool, a ceiling) is not read: giving it back says the chat may go on. The person's alone: no tool of
/// the app or the chat calls it. Blocks on SQLite.
pub fn give_back_in(root: &Path, task: TaskId) -> Result<bool, Message> {
  let stored = |error: StoreError| messages::store_error(&error);
  let mut db = ProjectDb::open(root).map_err(stored)?;
  let Some(parking) = db.parked_task(task).map_err(stored)? else {
    return Ok(false);
  };
  if let Some(run) = parking.run
    && !db.pending_tool_approvals(run).map_err(stored)?.is_empty()
  {
    return Err(Message::new("orchestrator.parking.requestOpen"));
  }
  Ok(db.unpark_task(task).map_err(stored)?.is_some())
}

/// The tasks parked for the person in the project at `root`, the one parked first first. Blocks on SQLite.
pub fn parked_in(root: &Path) -> Result<Vec<Parking>, Message> {
  let parked = || -> Result<Vec<Parking>, StoreError> { ProjectDb::open_read_only(root)?.parked_tasks() };
  parked().map_err(|error| messages::store_error(&error))
}
