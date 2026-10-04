//! The Conversation tab of a run that has not started (`FBwIV`): *Nothing in this conversation yet*,
//! and the steps the run will take in the rail, which a narrow window collapses to its strip
//! (`I35q7D`).

use std::{collections::BTreeMap, path::Path, time::Duration};

use orchester_core::{
  id::IdempotencyKey,
  resource::{AgentProfile, Pipeline, ResourceKind},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{agent_step, has, seed_steps},
  unreadable_run::name,
};

/// The project's published pipeline as a run pins it.
fn snapshot(db: &ProjectDb) -> RunSnapshot {
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
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

/// KON-1 with run 1 queued, on a pipeline of five steps whose names take more than the tab's width
/// at 960 on one line. No runner runs, so the run never starts.
fn seed(repo: &Path) {
  seed_steps(repo, &repo.join("no-cli"), |builder| {
    vec![
      agent_step(builder, "research", "Research the sign-in flow", "", &[]),
      agent_step(builder, "plan", "Plan the provider", "", &["research"]),
      agent_step(builder, "implement", "Implement the provider", "", &["plan"]),
      agent_step(builder, "checks", "Run the checks", "", &["implement"]),
      agent_step(builder, "review", "Review the change", "", &["checks"]),
    ]
  });
  let mut db = ProjectDb::open(repo).expect("open the project");
  let snapshot = snapshot(&db);
  let limits = TaskLimits::new(
    TokenBudget::new(1_000_000).expect("above zero"),
    Duration::from_secs(1_800),
  )
  .expect("limits");
  let task = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![TaskDetails {
        title: name("Add GitHub authentication"),
        brief: String::new(),
        acceptance_criteria: Vec::new(),
        pipeline: Some(snapshot.pipeline().resource),
        limits: Some(limits),
      }],
      Timestamp::now(),
    )
    .expect("task")
    .remove(0);
  db.create_run(
    IdempotencyKey::generate(),
    NewRun {
      task: task.id(),
      snapshot,
      branch: BranchName::new("orchester/kon-1-run-1").expect("branch"),
      base: Revision::new(&"a".repeat(40)).expect("revision"),
      base_branch: None,
      budget: limits.budget(),
      max_duration: limits.max_duration(),
    },
    Timestamp::now(),
  )
  .expect("queued run");
}

#[test]
fn conversation_queued_run_lists_its_steps_in_the_rail_strip_at_960() {
  let mut desktop = Desktop::with_seeded_project(seed);
  desktop.tree.resize(960, 960);
  desktop.pass();
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-tab-conversation");
  desktop.click("task-tab-conversation");
  desktop.wait_until("the queued run's empty chat", |desktop| {
    has(desktop, "conversation-empty")
  });
  assert!(shows(
    &mut desktop,
    "conversation-empty",
    "Nothing in this conversation yet"
  ));
  assert!(shows(
    &mut desktop,
    "conversation-empty",
    "Its steps are listed on the right"
  ));
  // At 960 the rail starts as its strip: a badge per step, none of them started yet.
  assert!(has(&mut desktop, "conversation-strip"));
  assert!(!has(&mut desktop, "conversation-rail"));
  assert!(shows(&mut desktop, "conversation-strip-step-5", "05"));
  // Opened from the strip, the rail shows over the chat and names the steps.
  desktop.click("conversation-rail-expand");
  desktop.wait_for("conversation-rail-over");
  assert!(shows(&mut desktop, "conversation-rail-step-5", "Review the change"));
  assert!(shows(&mut desktop, "conversation-rail-step-5", "Builder · not started"));
  assert!(shows(&mut desktop, "conversation-rail-all", "Queued · 5 steps"));
  let (strip, main) = (
    desktop.bounds("conversation-strip"),
    desktop.bounds("conversation-main"),
  );
  assert!(strip.x >= main.x + main.width - 0.5, "{strip:?} beside {main:?}");
}
