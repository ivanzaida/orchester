//! Story points kept beside tasks (ORC-49): a person's points are never replaced by automation's, even by a write
//! that raced the person's; an estimate of an earlier brief is not kept; a person's *Automatic* leaves automation's
//! points; points come back with the task; the
//! estimator's requests are their own kind of usage; and *Let Orchester estimate story points* is off until a person
//! turns it on. `cargo test -p orchester-store --test story_points story_points`.

mod support;

use orchester_core::{
  connection::AgentProvider,
  id::IdempotencyKey,
  resource::ModelId,
  run::TokenUsage,
  store::TaskStore,
  task::{
    TaskDetails, TaskUpdate,
    estimate::{EstimateBasis, EstimateRecord, EstimateSource, StoryEstimate, StoryPoints},
  },
  time::Timestamp,
};
use orchester_store::{AppDb, AutomationPoints};
use support::{at, started_run};

fn points(value: u32) -> StoryPoints {
  StoryPoints::new(value).expect("on the scale")
}

#[test]
fn story_points_of_a_person_are_never_replaced_by_automation() {
  let (_repo, mut db, run, _) = started_run();
  let task = db.task(run.task()).expect("read").expect("task");
  let basis = EstimateBasis::of(task.details());
  let automation = StoryEstimate::automation(points(3), Some("Small."), basis.clone(), at(1));
  assert_eq!(
    db.set_automation_points(task.id(), &automation).expect("stored"),
    AutomationPoints::Stored
  );
  let read = db.task(task.id()).expect("read").expect("task");
  assert_eq!(read.estimate(), Some(&automation), "points come back with the task");

  db.set_person_points(task.id(), Some(points(8)), at(2))
    .expect("the person's");
  let later = StoryEstimate::automation(points(1), None, basis, at(3));
  assert_eq!(
    db.set_automation_points(task.id(), &later)
      .expect("refused, not failed"),
    AutomationPoints::PersonsValue,
    "automation never writes over a person's points"
  );
  let estimate = db.story_estimate(task.id()).expect("read").expect("points");
  assert_eq!(
    (estimate.points(), estimate.source(), estimate.reason()),
    (points(8), EstimateSource::Person, None)
  );

  db.set_person_points(task.id(), None, at(4)).expect("Automatic");
  assert_eq!(db.story_estimate(task.id()).expect("read"), None);
  assert_eq!(
    db.set_automation_points(task.id(), &later).expect("stored"),
    AutomationPoints::Stored
  );
  db.set_person_points(task.id(), None, at(5)).expect("Automatic again");
  assert_eq!(
    db.story_estimate(task.id())
      .expect("read")
      .map(|estimate| estimate.points()),
    Some(points(1)),
    "Automatic keeps automation's points"
  );
}

#[test]
fn story_points_made_from_an_earlier_brief_are_not_kept() {
  let (_repo, mut db, run, _) = started_run();
  let task = db.task(run.task()).expect("read").expect("task");
  let draft = |brief: &str| TaskDetails {
    brief: brief.to_owned(),
    ..task.details().clone()
  };
  let created = db
    .create_tasks(IdempotencyKey::generate(), vec![draft("Brief A.")], at(1))
    .expect("task")
    .remove(0);
  let basis_a = EstimateBasis::of(created.details());
  let TaskUpdate::Saved(edited) = db
    .update_task(created.id(), created.revision(), draft("Brief B."))
    .expect("edited")
  else {
    panic!("no conflict");
  };
  let basis_b = EstimateBasis::of(edited.details());

  let of_b = StoryEstimate::automation(points(5), Some("From B."), basis_b, at(2));
  assert_eq!(
    db.set_automation_points(edited.id(), &of_b).expect("stored"),
    AutomationPoints::Stored
  );
  let of_a = StoryEstimate::automation(points(2), Some("From A."), basis_a, at(3));
  assert_eq!(
    db.set_automation_points(edited.id(), &of_a)
      .expect("not kept, not failed"),
    AutomationPoints::Outdated,
    "an estimate of an earlier brief that ends last does not replace the current one"
  );
  assert_eq!(db.story_estimate(edited.id()).expect("read"), Some(of_b));
}

#[test]
fn story_points_estimator_usage_is_its_own_kind_and_remembers_what_was_asked() {
  let (_repo, mut db, run, _) = started_run();
  let task = db.task(run.task()).expect("read").expect("task");
  let basis = EstimateBasis::of(task.details());
  assert!(!db.estimate_asked(task.id(), &basis).expect("asked"));
  let record = |usage| EstimateRecord {
    task: task.id(),
    basis: basis.clone(),
    provider: AgentProvider::ClaudeCodeCli,
    model: ModelId::new("claude-haiku-4-5-20251001").expect("model"),
    started_at: at(10),
    ended_at: at(11),
    usage,
    failure: None,
  };
  let usage = TokenUsage {
    input: 100,
    cached_input: 40,
    output: 9,
  };
  db.record_estimate_usage(&record(Some(usage))).expect("recorded");
  db.record_estimate_usage(&record(None)).expect("unknown usage recorded");
  assert!(db.estimate_asked(task.id(), &basis).expect("asked"));
  let (spent, requests) = db
    .estimates_between(at(0), Timestamp::from_unix_millis(i64::MAX))
    .expect("estimates");
  assert_eq!(
    (spent, requests),
    (usage, 2),
    "unknown usage counts as a request, not as tokens"
  );
  assert_eq!(db.estimates_between(at(12), at(20)).expect("later").1, 0);
}

#[test]
fn story_points_estimates_are_off_until_a_person_turns_them_on() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  assert!(!db.estimates_story_points().expect("read"), "off by default");
  db.set_estimates_story_points(true).expect("on");
  assert!(db.estimates_story_points().expect("read"));
}

/// The project's own setting (ORC-103) is off until a person sets it in Project settings › Runs.
#[test]
fn story_points_project_setting_is_off_until_a_person_sets_it() {
  let (_repo, mut db, ..) = started_run();
  assert!(!db.estimates_story_points().expect("read"));
  db.set_estimates_story_points(true).expect("on");
  assert!(db.estimates_story_points().expect("read"));
  db.set_estimates_story_points(false).expect("off");
  assert!(!db.estimates_story_points().expect("read"));
}

/// A project with no value of its own gets the application-wide one it moved from (ORC-595), once: later copies
/// neither read it nor change the project's.
#[test]
fn story_points_project_without_a_value_gets_the_application_wide_one_once() {
  let (_repo, mut db, ..) = started_run();
  db.copy_estimates_story_points(|| Ok(true)).expect("copied");
  assert!(db.estimates_story_points().expect("read"), "the application-wide value");
  db.copy_estimates_story_points(|| panic!("read again after the copy"))
    .expect("nothing to copy");
  assert!(db.estimates_story_points().expect("read"));
}

/// A project a person set the value for keeps it: the application-wide value is never read for it.
#[test]
fn story_points_project_with_its_own_value_keeps_it() {
  let (_repo, mut db, ..) = started_run();
  db.set_estimates_story_points(false).expect("off");
  db.copy_estimates_story_points(|| panic!("read for a project with its own value"))
    .expect("nothing to copy");
  assert!(!db.estimates_story_points().expect("read"));
}
