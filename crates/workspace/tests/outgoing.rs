//! A message written now, prepared and stored for the Conversation tab and automation alike: the task is
//! read again when it is sent, so a message written for a run that is no longer the latest is refused; a
//! press refuses to do anything else than it did when the person saw it (a plan that arrived, a run that was
//! canceled, the next version of a plan); it is stored as the latest run's state offers then, once per key;
//! a second way is refused where the state has one only; a plan's changes need a plan that waits.

mod support;

use orchester_core::{
  conversation::{Recipient, Timing, Undelivered},
  id::IdempotencyKey,
  run::{Answerer, PlanDecision, ReviewNote},
  store::ConversationStore,
};
use orchester_workspace::tasks::{
  conversation::{
    SendError,
    composer::{ComposerMode, Then},
    outgoing::{Action, Context, OutgoingError, Way, after_turn_run, plan_changes, prepare_in, store_in},
    send_after_turn_in, send_in,
  },
  detail::TaskDetail,
};

use self::support::{Fixture, at};

#[test]
fn outgoing_a_message_for_a_run_that_is_no_longer_the_latest_is_refused() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let run = fixture.started_run(task, at(20));
  let root = fixture.repo.path().to_path_buf();
  let stale = prepare_in(&root, (task, None), (false, None));
  assert!(
    matches!(stale, Err(OutgoingError::NotLatestRun(Some(number))) if number == run.number().get()),
    "written before the task had a run"
  );
  assert!(prepare_in(&root, (task, Some(run.id())), (false, None)).is_ok());
}

#[test]
fn outgoing_stores_as_the_state_offers_once_per_key() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  fixture.succeed(&mut run, at(30));
  let root = fixture.repo.path().to_path_buf();
  let prepared = prepare_in(&root, (task, Some(run.id())), (false, None)).expect("prepared");
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
    plan_changes(&prepared, "Change it.", (Answerer::Person, None)),
    Err(OutgoingError::NoPlan)
  ));
}

#[test]
fn outgoing_a_send_pressed_before_a_plan_arrived_does_not_request_its_changes() {
  let mut fixture = Fixture::gated();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let root = fixture.repo.path().to_path_buf();
  let before = prepare_in(&root, (task, Some(run.id())), (false, None)).expect("prepared");
  let pressed = Action::of(&before.mode, Way::Primary).expect("Send does something");
  assert_eq!(pressed, Action::Deliver, "{:?}", before.mode);
  fixture.propose(&mut run, "1. Replace sign-in.", at(30));
  let now = prepare_in(&root, (task, Some(run.id())), (false, None)).expect("prepared");
  assert!(matches!(now.mode, ComposerMode::PlanChanges { .. }));
  assert!(matches!(now.check(Way::Primary, pressed), Err(OutgoingError::Changed)));
}

#[test]
fn outgoing_a_send_pressed_before_the_run_was_canceled_does_not_run_it_again() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let root = fixture.repo.path().to_path_buf();
  let before = prepare_in(&root, (task, Some(run.id())), (false, None)).expect("prepared");
  let pressed = Action::of(&before.mode, Way::Primary).expect("Send does something");
  let events = run.cancel(at(30)).expect("canceled");
  fixture.record(&run, &events);
  let now = prepare_in(&root, (task, Some(run.id())), (false, None)).expect("prepared");
  assert_eq!(now.mode, ComposerMode::RunAgain);
  assert!(matches!(now.check(Way::Primary, pressed), Err(OutgoingError::Changed)));
  assert!(
    now.check(Way::Primary, Action::RunAgain).is_ok(),
    "pressed as it is offered, it goes"
  );
}

#[test]
fn outgoing_changes_written_for_one_plan_do_not_land_on_the_next() {
  let mut fixture = Fixture::gated();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let mut first = fixture.propose(&mut run, "1. Replace sign-in.", at(30));
  let root = fixture.repo.path().to_path_buf();
  let seen = prepare_in(&root, (task, Some(run.id())), (false, None)).expect("prepared");
  let ComposerMode::PlanChanges { version: v1, .. } = seen.mode else {
    panic!("a plan waits: {:?}", seen.mode);
  };
  let note = ReviewNote::new("Keep email sign-in.").expect("a note");
  let changes = PlanDecision::ChangesRequested { note };
  fixture.decide(&mut run, &mut first, (changes, Answerer::Automation), at(40));
  fixture.propose(&mut run, "1. Add GitHub beside email.", at(50));
  let now = prepare_in(&root, (task, Some(run.id())), (false, None)).expect("prepared");
  assert!(matches!(
    now.check(Way::Primary, Action::PlanChanges(v1)),
    Err(OutgoingError::Changed)
  ));
  assert!(matches!(
    plan_changes(&now, "Also log it.", (Answerer::Person, Some(v1))),
    Err(OutgoingError::Changed)
  ));
  // A question or plan the person left is the one they saw: a new plan offers its own mode again.
  let left = prepare_in(&root, (task, Some(run.id())), (false, Some(Context::Plan(v1)))).expect("prepared");
  assert!(matches!(left.mode, ComposerMode::PlanChanges { .. }));
}

#[test]
fn outgoing_a_message_of_an_ended_run_is_not_sent_after_the_turn() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let root = fixture.repo.path().to_path_buf();
  let mut refused = send_in(
    &root,
    IdempotencyKey::generate(),
    (task, Some(run.id()), None),
    (Recipient::Orchestrator, Timing::Now),
    "Use the API.",
  )
  .expect("stored");
  refused
    .not_delivered(run.id(), execution, at(30), Undelivered::Refused)
    .expect("not delivered");
  fixture
    .db
    .save_deliveries(std::slice::from_ref(&refused))
    .expect("saved");
  let detail = TaskDetail::load_in(&root, task).expect("read");
  assert_eq!(after_turn_run(&detail, true, refused.id()), Some(run.id()));
  let events = run.cancel(at(40)).expect("canceled");
  fixture.record(&run, &events);
  assert!(matches!(
    send_after_turn_in(&root, task, refused.id()),
    Err(SendError::NotReadable)
  ));
}
