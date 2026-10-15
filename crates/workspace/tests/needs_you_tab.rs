//! The task list's *Needs you* tab: it counts the tasks it lists, and what waits and is no task's — the
//! project's memory records, a tool request whose run cannot be read — is named in one line over its list
//! (`attention::outside_tasks`). Every other *Needs you* counts it ([`ProjectTasks::needs_you`]).

mod support;

use orchester_core::{
  integration::{MergeKind, RunMerge},
  memory::MemoryStore,
  resource::ModelId,
  revision::Revision,
  run::{BranchName, PendingToolApproval, Run},
  store::{ActivityStore, MergeStore},
};
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::{
  attention,
  data::ProjectTasks,
  list::{self, TaskTab},
};
use support::{Fixture, at, proposed};

fn load(fixture: &Fixture) -> ProjectTasks {
  ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("load the project's tasks")
}

/// A task whose run passed every step, merged at `now`: nothing of it waits.
fn merged(fixture: &mut Fixture, title: &str, now: i64) -> Run {
  let task = fixture.task(title, at(now));
  let mut run = fixture.started_run(task, at(now + 1));
  fixture.succeed(&mut run, at(now + 2));
  let receipt = RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("a branch"),
    revision: Revision::new(&"b".repeat(40)).expect("a revision"),
    kind: MergeKind::FastForward,
    merged_at: at(now + 3),
  };
  fixture.db.save_run_merge(&receipt).expect("record the merge");
  run
}

fn titles(tasks: &ProjectTasks) -> Vec<String> {
  list::rows(&tasks.tasks, tasks, TaskTab::NeedsYou)
    .iter()
    .map(|row| row.title.to_string())
    .collect()
}

#[test]
fn needs_you_tab_with_only_memory_waiting_counts_no_task_and_names_the_memory() {
  let mut fixture = Fixture::new();
  let run = merged(&mut fixture, "Add sign-out", 10);
  fixture
    .db
    .add_memories(&[proposed(&run, "Sign-out clears the cookie.", 40)])
    .expect("the record waits for approval");
  let tasks = load(&fixture);
  assert_eq!(tasks.needs_you(), 1, "the pill counts the record");
  assert_eq!(TaskTab::NeedsYou.count(&tasks), 0, "no task waits");
  assert!(titles(&tasks).is_empty());
  let named = attention::outside_tasks(&tasks).expect("the record is named over the list");
  assert_eq!(named.key, "overview.needs.title.memoryOne");
}

#[test]
fn needs_you_tab_counts_the_waiting_task_it_lists_and_names_the_memory_beside_it() {
  let mut fixture = Fixture::new();
  let passed = fixture.task("Write docs", at(10));
  let mut done = fixture.started_run(passed, at(20));
  fixture.succeed(&mut done, at(30));
  let records = [
    proposed(&done, "Docs live in docs/.", 40),
    proposed(&done, "Examples build in CI.", 50),
  ];
  fixture
    .db
    .add_memories(&records)
    .expect("the records wait for approval");
  let tasks = load(&fixture);
  assert_eq!(
    tasks.needs_you(),
    2,
    "the pill counts the work to merge and the records"
  );
  assert_eq!(titles(&tasks), ["Write docs"]);
  assert_eq!(TaskTab::NeedsYou.count(&tasks), 1, "the tab counts what it lists");
  let named = attention::outside_tasks(&tasks).expect("the records are named over the list");
  assert_eq!(named.key, "overview.needs.title.memoryOther");
  assert_eq!(named.args, [("count", "2".to_owned())]);
}

#[test]
fn needs_you_tab_names_nothing_when_everything_waiting_is_a_task_s() {
  let mut fixture = Fixture::new();
  let passed = fixture.task("Write docs", at(10));
  let mut done = fixture.started_run(passed, at(20));
  fixture.succeed(&mut done, at(30));
  let tasks = load(&fixture);
  assert_eq!(TaskTab::NeedsYou.count(&tasks), tasks.needs_you());
  assert_eq!(attention::outside_tasks(&tasks), None);
}

#[test]
fn needs_you_tab_counts_what_waits_outside_the_tasks_when_several_do() {
  let mut fixture = Fixture::keyed("KON");
  let shipped = merged(&mut fixture, "Add sign-out", 10);
  let task = fixture.task("Add login", at(20));
  let run = fixture.started_run(task, at(21));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(22))
    .expect("invocation")
    .id;
  let approval = PendingToolApproval {
    id: format!("{invocation}:call_1"),
    run: run.id(),
    execution,
    invocation,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: String::new(),
    action: "pnpm build".to_owned(),
    target: None,
    requested_at: at(23),
    native: None,
  };
  fixture.db.request_tool_approval(&approval).expect("the request waits");
  fixture
    .db
    .add_memories(&[proposed(&shipped, "Sign-out clears the cookie.", 40)])
    .expect("the record waits for approval");
  // The store has no API for a damaged record: an edit outside Orchester leaves it.
  let changed = rusqlite::Connection::open(fixture.repo.path().join(".orchester").join("orchester.db"))
    .expect("the project database")
    .execute(
      "UPDATE runs SET snapshot = 'not json' WHERE id = ?1",
      [run.id().to_string()],
    )
    .expect("damage the run");
  assert_eq!(changed, 1);
  let tasks = load(&fixture);
  assert_eq!(tasks.needs_you(), 2, "the request and the record");
  assert_eq!(
    TaskTab::NeedsYou.count(&tasks),
    0,
    "the request's task is not known, so the tab lists no task"
  );
  let named = attention::outside_tasks(&tasks).expect("both are named over the list");
  assert_eq!(named.key, "tasks.needsYou.outsideTasks");
  assert_eq!(named.args, [("count", "2".to_owned())]);
}
