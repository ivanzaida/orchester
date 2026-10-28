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

#[test]
fn task_whose_run_succeeded_with_work_to_merge_only_moves_to_todo_since_it_never_runs_again() {
  let mut repo = Repo::new();
  let failed = repo.task("Sign in");
  repo.failed(&failed, false);
  let succeeded = repo.task("Sign out");
  repo.succeeded(&succeeded);

  assert!(row(&repo, failed.id()).runs_again, "a failed run is run again");
  let ready = row(&repo, succeeded.id());
  assert!(
    !ready.runs_again,
    "its work waits to be merged: core's run_again refuses it"
  );
  assert!(!ready.run_not_ended);
  // No Run again to offer: the drop on Todo is the move only, asked about by nobody.
  assert_eq!(
    moves::decide(&ready, WorkflowStatus::Todo),
    Decision::Move(moves::MOVE_ONLY)
  );
}

#[test]
fn task_whose_interrupted_run_lost_its_worktree_is_asked_to_run_again_on_todo() {
  let mut repo = Repo::new();
  let interrupted = repo.task("Sign in");
  repo.interrupted_without_worktree(&interrupted);
  let kept = repo.task("Sign out");
  let mut run = repo.working(&kept, false);
  let events = run
    .interrupt(orchester_core::run::InterruptCause::Crashed, super::at(9))
    .expect("interrupt");
  repo.db.append_events(&run, &events).expect("events");

  // It never continues, so it no longer holds its task, and core's run_again follows it.
  let lost = row(&repo, interrupted.id());
  assert!(!lost.run_not_ended, "it no longer holds its task");
  assert!(lost.runs_again, "core's can_run_again_after takes it");
  assert_eq!(
    moves::decide(&lost, WorkflowStatus::Todo),
    Decision::Move(Move::RunAgain)
  );
  // With its worktree it continues there: not ended, the board refuses it on Todo.
  let continues = row(&repo, kept.id());
  assert!(continues.run_not_ended);
  assert!(!continues.runs_again);
}
