//! update_task closing a task as the app does, and a task whose run ended moved to Todo by the board or over MCP.

use orchester_core::{
  id::TaskId,
  store::{RunStore, TaskStore},
  task::{
    TaskStatus,
    organization::{ChangeActor, WorkflowStatus},
  },
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, Translate},
  tasks::{
    data::ProjectTasks,
    integration_jobs::ProjectJobs,
    list::{TaskRow, TaskTab, rows},
    moves::{self, Decision, Move, write::set_status_in},
    outside::OutsideChanges,
  },
};
use serde_json::{Value, json};

use super::Repo;

/// The app's MCP tools on `repo`'s session.
fn tools(repo: &Repo) -> app_mcp::Services {
  app_mcp::Services::new(
    repo.workspace.clone(),
    repo.runners(),
    repo.servers(),
    Programs::processes(),
    (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
  )
}

fn call(repo: &Repo, services: &app_mcp::Services, name: &str, args: &Value) -> Result<Value, Value> {
  let translate = Translate::new(|message| message.key.to_owned());
  let Value::Object(args) = args.clone() else {
    panic!("arguments are an object");
  };
  repo
    .runtime
    .block_on(app_mcp::call(services, &translate, name, args))
    .expect("a tool")
    .map_err(|error| error.to_json(|message| message.key.to_owned()))
}

#[test]
fn task_close_over_mcp_follows_the_app_and_no_tool_deletes_a_task() {
  let mut repo = Repo::new();
  let failed = repo.task("Sign in");
  repo.failed(&failed, false);
  let working = repo.task("Sign out");
  repo.working(&working, false);
  let services = tools(&repo);

  let closed = call(
    &repo,
    &services,
    "update_task",
    &json!({ "project": "Kontur", "key": failed.key().to_string(), "workflow_status": "canceled" }),
  )
  .expect("closed");
  assert_eq!(closed["task"]["workflow_status"], "canceled");
  for status in ["done", "canceled"] {
    let refused = call(
      &repo,
      &services,
      "update_task",
      &json!({ "project": "Kontur", "key": working.key().to_string(), "workflow_status": status }),
    )
    .expect_err("its run has not ended");
    assert_eq!(refused["error"]["code"], "stop_run_first", "{status}");
  }
  assert_eq!(repo.status(working.id()), WorkflowStatus::InProgress);
  assert!(
    app_mcp::listed().iter().all(|tool| !tool.name.contains("delete_task")),
    "deleting a task is a person's"
  );
}

/// The board's row of `task`, as the Tasks board decides its drops with it.
fn row(repo: &Repo, task: TaskId) -> TaskRow {
  let tasks = ProjectTasks::load(repo.root(), &Programs::processes()).expect("the tasks read");
  rows(&tasks.tasks, &tasks, TaskTab::All)
    .into_iter()
    .find(|row| row.id == task)
    .expect("its row")
}

/// What the task has: its submission and its runs.
fn standing(repo: &Repo, task: TaskId) -> (TaskStatus, usize) {
  let db = ProjectDb::open(repo.root()).expect("open");
  let stored = db.task(task).expect("read").expect("the task");
  (stored.status(), db.runs_of_task(task).expect("runs").len())
}

#[test]
fn task_whose_run_ended_moves_to_todo_only_and_starts_nothing_on_the_board_and_over_mcp() {
  let mut repo = Repo::new();
  let failed = repo.task("Sign in");
  repo.failed(&failed, false);
  let other = repo.task("Sign out");
  repo.failed(&other, false);
  // Dropped on Todo, a task whose run ended is asked: run again, or move only, which is the status alone.
  let moved = row(&repo, failed.id());
  assert_eq!(
    moves::decide(&moved, WorkflowStatus::Todo),
    Decision::Move(Move::RunAgain)
  );
  assert_eq!(moves::MOVE_ONLY, Move::Status(WorkflowStatus::Todo));
  let before = standing(&repo, failed.id());

  // The board's Move to Todo.
  set_status_in(
    repo.root(),
    &repo.servers(),
    failed.id(),
    (WorkflowStatus::Todo, ChangeActor::Person),
  )
  .expect("moved");
  assert_eq!(repo.status(failed.id()), WorkflowStatus::Todo);
  assert_eq!(standing(&repo, failed.id()), before, "no run, nothing queued");
  let row_now = row(&repo, failed.id());
  assert_eq!(row_now.phase, moved.phase, "its card keeps its last run's outcome");
  assert!(!row_now.run_not_ended);
  assert_eq!(
    moves::decide(&row_now, WorkflowStatus::Todo),
    Decision::Stay,
    "in Todo already"
  );

  // update_task workflow_status todo does the same.
  let before = standing(&repo, other.id());
  let services = tools(&repo);
  let changed = call(
    &repo,
    &services,
    "update_task",
    &json!({ "project": "Kontur", "key": other.key().to_string(), "workflow_status": "todo" }),
  )
  .expect("moved");
  assert_eq!(changed["task"]["workflow_status"], "todo");
  assert_eq!(standing(&repo, other.id()), before, "no run, nothing queued");
  assert_eq!(row(&repo, other.id()).phase, row_now.phase);
}
