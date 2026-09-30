//! Current-candidate regressions for tab routing, pinned row labels and held Pending recovery.
use std::{collections::BTreeMap, path::Path, process::Command};

use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  resource::{AgentProfile, Pipeline, ResourceKind},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot, RunStatus},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  task_group::{GroupHold, ReviewedPipeline, Scheduling, TaskGroup},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{init_repository, programs, seed},
};

fn snapshot(db: &ProjectDb, pipeline: orchester_core::id::ResourceId) -> RunSnapshot {
  let version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("version")
    .expect("published");
  let profiles = db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .iter()
    .map(|resource| {
      (
        resource.id(),
        db.latest_version::<AgentProfile>(resource.id())
          .expect("version")
          .expect("published"),
      )
    })
    .collect();
  let (tools, instructions, skills) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
  RunSnapshot::build(
    pipeline,
    &version,
    PublishedVersions {
      profiles: &profiles,
      tools: &tools,
      instructions: &instructions,
      skills: &skills,
    },
  )
  .expect("snapshot")
}

fn pending(db: &mut ProjectDb, repo: &Path, task: &orchester_core::task::Task, snapshot: &RunSnapshot) -> Run {
  let head = Command::new("git")
    .args(["rev-parse", "HEAD"])
    .current_dir(repo)
    .output()
    .expect("git");
  assert!(head.status.success());
  let base = Revision::new(String::from_utf8(head.stdout).expect("utf8").trim()).expect("revision");
  let limits = task.details().limits.expect("limits");
  db.create_run(
    task.id().first_run_key(),
    NewRun {
      task: task.id(),
      snapshot: snapshot.clone(),
      branch: BranchName::for_next_run(task.key(), 0),
      base,
      base_branch: None,
      budget: limits.budget(),
      max_duration: limits.max_duration(),
    },
    Timestamp::now(),
  )
  .expect("first run")
}

fn fixture(repo: &Path, fake: &Path) {
  seed(repo, fake, false);
  let mut db = ProjectDb::open(repo).expect("db");
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let snapshot = snapshot(&db, pipeline);
  let limits = TaskLimits::new(
    TokenBudget::new(1_400_000).expect("budget"),
    std::time::Duration::from_secs(1800),
  )
  .expect("limits");
  let details = ["Z Queue target", "B Review target", "C Done target", "A Draft target"]
    .into_iter()
    .map(|title| TaskDetails {
      title: Name::new(title).expect("name"),
      brief: "Append one line.".into(),
      acceptance_criteria: vec!["One line is appended.".into()],
      pipeline: Some(pipeline),
      limits: Some(limits),
    })
    .collect();
  let tasks = db
    .create_tasks(IdempotencyKey::generate(), details, Timestamp::now())
    .expect("drafts");
  for task in &tasks[..3] {
    let group = TaskGroup::new(
      IdempotencyKey::generate(),
      vec![task.id()],
      Scheduling::Sequential,
      TokenBudget::new(2_000_000).expect("cap"),
    )
    .expect("group");
    db.queue_task_group(
      &group,
      None,
      &[ReviewedPipeline {
        task: task.id(),
        pipeline,
        version: snapshot.pipeline().number,
      }],
    )
    .expect("queue");
    let mut run = pending(&mut db, repo, task, &snapshot);
    if task.id() == tasks[1].id() {
      let events = run.start(Timestamp::now()).expect("start");
      db.append_events(&run, &events).expect("events");
      let step = run.current_attempt().expect("attempt").executions()[0].id();
      let events = run.start_step(step, Timestamp::now()).expect("step");
      db.append_events(&run, &events).expect("events");
      let events = run.complete_step(step, Timestamp::now()).expect("complete");
      db.append_events(&run, &events).expect("events");
    } else if task.id() == tasks[2].id() {
      let events = run.cancel(Timestamp::now()).expect("cancel");
      db.append_events(&run, &events).expect("events");
    }
  }
  let version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("version")
    .expect("published");
  let mut steps = version.body().steps().to_vec();
  steps[0].name = Name::new("Unseen new step").expect("name");
  let mut draft = version.to_draft(Timestamp::now());
  draft.edit(Pipeline::new(steps).expect("pipeline"), Timestamp::now());
  db.save_draft(pipeline, &draft).expect("draft");
  db.publish_draft::<Pipeline>(pipeline, Timestamp::now())
    .expect("publish");
}

fn desktop() -> Desktop {
  let (runner, fake) = programs();
  Desktop::with_runner(runner, init_repository, |repo| fixture(repo, &fake))
}

fn pick(desktop: &mut Desktop, id: &str, steps: usize) {
  desktop.tab_to(id, 40);
  desktop.enter();
  for _ in 0..steps {
    desktop.key("ArrowDown", "ArrowDown");
  }
  desktop.enter();
}

#[test]
fn task_bulk_tab_navigation_preserves_queue_review_done_with_combined_filters() {
  let mut desktop = desktop();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.tab_to("tasks-search", 40);
  desktop.type_text("target");
  pick(&mut desktop, "tasks-pipeline-filter", 1);
  pick(&mut desktop, "tasks-sort", 2);
  for (tab, number) in [
    ("queue", 1),
    ("needs-review", 2),
    ("done", 3),
    ("drafts", 4),
    ("queue", 1),
  ] {
    desktop.click(&format!("tasks-tab-{tab}"));
    desktop.wait_until("only the navigated tab's matching row", |desktop| {
      (1..=4).all(|row| desktop.tree.get_element_by_id(&format!("task-row-KON-{row}")).is_some() == (row == number))
    });
    assert!(
      desktop
        .tree
        .get_element_by_id_mut("tasks-pipeline-filter")
        .and_then(|e| e.as_select())
        .expect("filter")
        .is_selected(1)
    );
    assert!(
      desktop
        .tree
        .get_element_by_id_mut("tasks-sort")
        .and_then(|e| e.as_select())
        .expect("sort")
        .is_selected(2)
    );
  }
  desktop.click("tasks-tab-all");
  desktop.wait_for("task-row-KON-4");
  assert!(desktop.bounds("task-row-KON-4").y < desktop.bounds("task-row-KON-1").y);
  desktop.tab_to("tasks-search", 40);
  desktop.key("End", "End");
  for _ in 0..6 {
    desktop.key("Backspace", "Backspace");
  }
  desktop.type_text("queue");
  for tab in ["needs-review", "done"] {
    desktop.click(&format!("tasks-tab-{tab}"));
    desktop.wait_until("combined query excludes other tabs", |desktop| {
      (1..=4).all(|row| desktop.tree.get_element_by_id(&format!("task-row-KON-{row}")).is_none())
    });
    assert_eq!(desktop.value("tasks-search"), "queue");
  }
  desktop.click("tasks-tab-queue");
  desktop.wait_for("task-row-KON-1");
}

#[test]
fn task_bulk_rows_use_reviewed_and_latest_run_versions_after_new_publication() {
  let mut desktop = desktop();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  for number in 1..=3 {
    assert!(shows(&mut desktop, &format!("task-row-KON-{number}"), "Delivery · v1"));
    assert!(!shows(&mut desktop, &format!("task-row-KON-{number}"), "Delivery · v2"));
  }
  assert!(shows(&mut desktop, "task-row-KON-4", "Delivery · v2"));
}

#[test]
fn task_bulk_fresh_pending_first_run_has_explicit_recovery_with_saved_identity() {
  let mut desktop = desktop();
  let db = ProjectDb::open(desktop.repo()).expect("db");
  let task = db.tasks().expect("tasks")[0].id();
  let original = db.run_with_key(task.first_run_key()).expect("first").expect("exists");
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-start-pending-run");
  assert!(shows(&mut desktop, "task-notice", "Scheduling is held after reopening"));
  assert!(shows(&mut desktop, "task-heading", "scheduling held after reopening"));
  desktop.click("task-breadcrumb");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-start-pending-run");
  assert_eq!(db.run(original.id()).expect("run").expect("exists"), original);
  desktop.click("task-start-pending-run");
  desktop.wait_until("the same first run succeeds", |_| {
    db.run(original.id()).expect("run").expect("exists").status() == &RunStatus::Succeeded
  });
  let recovered = db.run_with_key(task.first_run_key()).expect("first").expect("exists");
  assert_eq!(recovered.id(), original.id());
  assert_eq!(recovered.snapshot(), original.snapshot());
  assert_eq!(recovered.branch(), original.branch());
  assert_eq!(db.runs_of_task(task).expect("runs").len(), 1);
  let group = db.task_group_for_task(task).expect("group").expect("exists");
  assert_ne!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::SessionEnded)
  );
}
