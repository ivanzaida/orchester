//! A task whose latest run cannot be read (edited or damaged outside Orchester): the task list
//! loads and names the run *Unreadable* (`mGbYs`), and the task page (02.R `SUuwV`) says why,
//! shows its actions unavailable and lists the task's other runs.

use std::{collections::BTreeMap, path::Path, time::Duration};

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, TaskId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;
use rusqlite::Connection;

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
};

pub(crate) fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

pub(crate) fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// The published one-step pipeline "Delivery" of `db`, as a run pins it.
pub(crate) fn delivery(db: &mut ProjectDb) -> RunSnapshot {
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("valid model"),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(1))
    .expect("profile")
    .id();
  let profiles = BTreeMap::from([(
    builder,
    db.publish_draft::<AgentProfile>(builder, at(2))
      .expect("publish the profile"),
  )]);
  let step = Step {
    key: StepKey::new("implement").expect("valid key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("valid pipeline");
  let delivery = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, at(3))
    .expect("pipeline")
    .id();
  let version = db
    .publish_draft::<Pipeline>(delivery, at(4))
    .expect("publish the pipeline");
  let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &instructions,
    skills: &std::collections::BTreeMap::new(),
  };
  RunSnapshot::build(delivery, &version, published).expect("snapshot")
}

/// Run `number` of `task`, started at `now`.
pub(crate) fn started(db: &mut ProjectDb, (task, snapshot): (TaskId, &RunSnapshot), number: u32, now: i64) -> Run {
  let new = NewRun {
    task,
    snapshot: snapshot.clone(),
    branch: BranchName::new(&format!("orchester/kon-1-run-{number}")).expect("branch"),
    base: Revision::new(&"a".repeat(40)).expect("revision"),
    base_branch: None,
    budget: TokenBudget::new(1_000_000).expect("above zero"),
    max_duration: Duration::from_secs(1_800),
  };
  let mut run = db.create_run(IdempotencyKey::generate(), new, at(now)).expect("run");
  let events = run.start(at(now)).expect("start");
  db.append_events(&run, &events).expect("append");
  run
}

/// KON-1 "Add repository switcher" with run #1, cancelled, and run #2, whose record says it is
/// pending although it started: a record Orchester cannot read back.
fn seed(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open the project database");
  let snapshot = delivery(&mut db);
  let details = TaskDetails {
    title: name("Add repository switcher"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: Some(
      TaskLimits::new(
        TokenBudget::new(1_000_000).expect("above zero"),
        Duration::from_secs(1_800),
      )
      .expect("limits"),
    ),
  };
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(5))
    .expect("task")[0]
    .id();
  let mut first = started(&mut db, (task, &snapshot), 1, 10);
  let events = first.cancel(at(20)).expect("cancel");
  db.append_events(&first, &events).expect("append");
  let second = started(&mut db, (task, &snapshot), 2, 30);
  Connection::open(repo.join(".orchester/orchester.db"))
    .expect("open raw database")
    .execute(
      "UPDATE runs SET status = 'pending' WHERE id = ?1",
      [second.id().to_string()],
    )
    .expect("tamper");
}

fn holds(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  desktop
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| contains_text(element, text))
}

#[test]
fn unreadable_run_shows_in_the_task_list_and_on_its_task_page() {
  let mut desktop = Desktop::with_seeded_project(seed);
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_until("the task list with the unreadable run", |desktop| {
    holds(desktop, "task-row-KON-1", "Unreadable")
  });
  assert!(shows(&mut desktop, "task-row-KON-1", "Run #2 could not be read"));

  desktop.click("task-row-KON-1");
  desktop.wait_until("the unreadable run's notice", |desktop| {
    holds(desktop, "task-unreadable-notice", "Run #2 could not be read")
  });
  assert!(shows(
    &mut desktop,
    "task-unreadable-notice",
    "The database file is kept unchanged"
  ));
  assert!(shows(&mut desktop, "task-page", "Run 2 · could not be read"));
  assert!(shows(
    &mut desktop,
    "task-unreadable-actions",
    "are unavailable for run #2 until it can be read"
  ));
  assert!(shows(&mut desktop, "task-other-runs", "Run #1"));
  assert!(shows(&mut desktop, "task-other-runs", "Cancelled"));
  // Cancel run is shown, unavailable: pressing it asks the runner nothing.
  assert!(shows(&mut desktop, "task-cancel-run", "Cancel run"));
  desktop.click("task-cancel-run");
  assert!(shows(
    &mut desktop,
    "task-unreadable-notice",
    "Run #2 could not be read"
  ));

  for (tab, state) in [
    ("task-tab-conversation", "conversation-unreadable"),
    ("task-tab-changes", "changes-unreadable"),
    ("task-tab-activity", "activity-unreadable"),
  ] {
    desktop.click(tab);
    desktop.wait_until(state, |desktop| holds(desktop, state, "Run #2 could not be read"));
  }
}
