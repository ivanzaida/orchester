//! What follows a turn Orchester started by itself that failed (`orchestrator::retry`): a failure that may pass by
//! itself is tried again after one, five and fifteen minutes, and a fourth is left to the person; a failure only the
//! person can lift is left to them at once; a turn nobody recorded the end of is due at once; and the next turn is
//! told how the earlier one ended.
//! `cargo test -p orchester-core --test orchestrator_retry retry`.

use orchester_core::{
  connection::AccountStartProblem,
  orchestrator::{FailedTry, NextTry, TurnFailure, retry_note},
  time::Timestamp,
};

const MINUTE: i64 = 60_000;

fn failed(failure: TurnFailure, ended: i64) -> FailedTry {
  FailedTry {
    failure,
    ended: Some(Timestamp::from_unix_millis(ended)),
  }
}

fn passing() -> Vec<TurnFailure> {
  vec![
    TurnFailure::CannotStart {
      reason: "the runner did not take it".to_owned(),
    },
    TurnFailure::Failed {
      reason: "it broke".to_owned(),
    },
    TurnFailure::Interrupted,
    TurnFailure::TimeLimit { minutes: 10 },
  ]
}

#[test]
fn retry_follows_a_failure_that_may_pass_after_one_five_and_fifteen_minutes() {
  for failure in passing() {
    let last = failed(failure, 1_000);
    for (earlier, minutes) in [(0, 1), (1, 5), (2, 15)] {
      let due = Timestamp::from_unix_millis(1_000 + minutes * MINUTE);
      let next = NextTry::after(&last, earlier);
      assert_eq!(next, NextTry::From(due), "{last:?} after {earlier} earlier");
      assert!(!next.is_due(Timestamp::from_unix_millis(due.unix_millis() - 1)));
      assert!(next.is_due(due));
    }
  }
}

#[test]
fn retry_leaves_a_fourth_failure_to_the_person() {
  for failure in passing() {
    let next = NextTry::after(&failed(failure, 1_000), 3);
    assert_eq!(next, NextTry::LeftToPerson);
    assert!(!next.is_due(Timestamp::from_unix_millis(i64::MAX)));
  }
}

#[test]
fn retry_leaves_what_only_the_person_can_lift_to_them_at_once() {
  let theirs = [
    TurnFailure::NotSignedIn,
    TurnFailure::ConnectionGone,
    TurnFailure::Account {
      problem: AccountStartProblem::NoAccount,
    },
  ];
  for failure in theirs {
    assert!(!failure.is_tried_again(), "{failure:?}");
    assert_eq!(NextTry::after(&failed(failure, 1_000), 0), NextTry::LeftToPerson);
  }
}

#[test]
fn retry_of_a_turn_nobody_recorded_the_end_of_is_due_at_once() {
  let cut_short = FailedTry {
    failure: TurnFailure::Interrupted,
    ended: None,
  };
  assert!(NextTry::after(&cut_short, 0).is_due(Timestamp::now()));
}

#[test]
fn retry_note_tells_the_next_turn_how_the_earlier_one_ended_and_to_read_the_state_first() {
  let note = retry_note(&failed(TurnFailure::TimeLimit { minutes: 10 }, 1_000));
  assert!(note.contains("ran longer than the 10 minutes"), "{note}");
  assert!(note.contains("do not repeat what is already done"), "{note}");
  let kept = serde_json::to_string(&failed(TurnFailure::Interrupted, 1_000)).expect("written");
  let read: FailedTry = serde_json::from_str(&kept).expect("read");
  assert_eq!(
    read,
    failed(TurnFailure::Interrupted, 1_000),
    "a failed turn is kept and read back"
  );
}
