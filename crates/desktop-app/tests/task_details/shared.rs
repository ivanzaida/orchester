//! A shared task a teammate ran on their computer: the Summary and the Details never take it for one without
//! runs.

use std::{collections::HashMap, num::NonZeroU32};

use orchester_app::routes::TaskTab;
use orchester_core::{
  id::{ProjectId, ServerId, UserId},
  run::TaskPhase,
  sharing::MemberRole,
  task::Task,
  time::Timestamp,
};
use orchester_store::{CachedRunStatus, CachedTask, ProjectDb, SharedPerson, SharingLink};

use crate::{Page, assert_shows};

/// Shares the project with `task` as its server has it: Alice Moreau ran it on her computer, run #7, which
/// works on Implement.
fn ran_on_a_teammates_computer(page: &Page, task: &Task) {
  let mut db = ProjectDb::open(&page.repo).expect("the project");
  let details = task.details();
  let alice = SharedPerson {
    id: UserId::generate(),
    name: "Alice Moreau".to_owned(),
  };
  let cached = CachedTask {
    id: task.id(),
    number: task.key().number(),
    revision: task.revision(),
    title: details.title.clone(),
    brief: details.brief.clone(),
    criteria: details.acceptance_criteria.clone(),
    pipeline: None,
    limits: details.limits,
    status: task.status(),
    organization: db.task_organization(task.id()).expect("organization"),
    module: None,
    former_key: None,
    created_by: alice.name.clone(),
    created_at: task.created_at(),
    updated_by: alice.name.clone(),
    updated_at: Timestamp::now(),
    claim: None,
    run: Some(CachedRunStatus {
      holder: alice,
      number: NonZeroU32::new(7).expect("a number"),
      phase: TaskPhase::Running,
      step: Some("Implement".into()),
      started_at: Timestamp::now(),
      phase_since: Timestamp::now(),
      updated_at: Timestamp::now(),
    }),
  };
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: task.key().project().clone(),
    role: Some(MemberRole::Member),
    joined_at: Timestamp::now(),
    cursor: 0,
    last_synced_at: Timestamp::now(),
    removed_at: None,
    address: Some("https://orchester.kontur.lan/kontur".into()),
  };
  db.start_sharing(&link, &[], (&[cached], &HashMap::new()))
    .expect("shared");
}

#[test]
fn summary_a_shared_task_a_teammate_ran_is_not_taken_for_no_runs() {
  let mut page = Page::seeded("task-states");
  let key = page.key("queued, run not started");
  let task = page.task(&key);
  ran_on_a_teammates_computer(&page, &task);
  page.open(&key, TaskTab::Summary, "task-event-text");
  assert!(!page.has("task-summary-empty"), "the task had a run");
  assert!(
    page
      .text("task-event-text")
      .starts_with("Run #7 on Alice Moreau’s computer · Running · Implement."),
    "{}",
    page.text("task-event-text")
  );
  assert_eq!(page.text("task-property-pipeline"), "Not recorded");
  page.choose("Current task");
  assert_shows(&mut page, task.details(), "a teammate's run");
}
