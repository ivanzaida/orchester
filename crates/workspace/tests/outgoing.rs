//! A message written now, prepared and stored for the Conversation tab and automation alike: the task is
//! read again when it is sent, so a message written for a run that is no longer the latest is refused; it is
//! stored as the latest run's state offers then, once per key; a second way is refused where the state has
//! one only; a plan's changes need a plan that waits.

mod support;

use orchester_core::{conversation::Timing, id::IdempotencyKey, run::Answerer};
use orchester_workspace::tasks::conversation::{
  composer::{ComposerMode, Then},
  outgoing::{OutgoingError, Way, plan_changes, prepare_in, store_in},
};

use self::support::{Fixture, at};

#[test]
fn outgoing_a_message_for_a_run_that_is_no_longer_the_latest_is_refused() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let run = fixture.started_run(task, at(20));
  let root = fixture.repo.path().to_path_buf();
  let stale = prepare_in(&root, (task, None), (false, false));
  assert!(
    matches!(stale, Err(OutgoingError::NotLatestRun(Some(number))) if number == run.number().get()),
    "written before the task had a run"
  );
  assert!(prepare_in(&root, (task, Some(run.id())), (false, false)).is_ok());
}

#[test]
fn outgoing_stores_as_the_state_offers_once_per_key() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  fixture.succeed(&mut run, at(30));
  let root = fixture.repo.path().to_path_buf();
  let prepared = prepare_in(&root, (task, Some(run.id())), (false, false)).expect("prepared");
  assert_eq!(prepared.mode, ComposerMode::FollowUp);
  let key = IdempotencyKey::generate();
  let (then, first) = store_in(&root, &prepared, (key, Way::Primary), "Add a test.").expect("stored");
  assert_eq!(then, Then::Deliver);
  assert_eq!(first.timing(), Timing::AfterTurn);
  assert_eq!(first.run(), Some(run.id()));
  let (_, again) = store_in(&root, &prepared, (key, Way::Primary), "Add a test.").expect("the first");
  assert_eq!(again.id(), first.id(), "a retry stores nothing new");
  assert!(matches!(
    store_in(&root, &prepared, (IdempotencyKey::generate(), Way::Second), "More."),
    Err(OutgoingError::NoSecondWay)
  ));
  assert!(matches!(
    plan_changes(&prepared, "Change it.", Answerer::Person),
    Err(OutgoingError::NoPlan)
  ));
}
