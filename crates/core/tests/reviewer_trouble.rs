//! Why auto mode's reviewer could not run (ORC-611): a CLI that failed on an account at its plan limit says the limit
//! and when it resets, on a signed-out account that nobody is signed in, otherwise that the CLI failed; a timeout and
//! an unreadable answer say so whatever the account. Project settings › Runs reads the reviewer's account and its last
//! review and estimate the same way.
//! `cargo test -p orchester-core --test reviewer_trouble reviewer_trouble`.

use orchester_core::{
  account::AccountState,
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits},
  },
  run::review::{ReviewFailure, ReviewerAccount, ReviewerHealth, ReviewerTrouble},
  time::Timestamp,
};

const WEEK: u64 = 7 * 24 * 60;

fn at(seconds: i64) -> Timestamp {
  Timestamp::from_unix_seconds(1_800_000_000 + seconds)
}

/// A reading at `at(0)` of the weekly window, `used` percent of it used, resetting at `at(resets)`.
fn reading(used: f64, state: LimitState, resets: i64) -> LimitReading {
  LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    account: None,
    at: at(0),
    limits: PlanLimits {
      windows: vec![LimitWindow::new(WEEK, Some(used), Some(at(resets)), state, false).expect("window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  }
}

fn signed_in(reading: Option<LimitReading>) -> ReviewerAccount {
  ReviewerAccount {
    state: Some(AccountState::SignedIn),
    reading,
  }
}

#[test]
fn reviewer_trouble_a_cli_that_fails_says_the_cli_failed() {
  let account = signed_in(Some(reading(40.0, LimitState::Normal, 3_600)));
  assert_eq!(
    ReviewerTrouble::of(ReviewFailure::CouldNotRun, &account, at(60)),
    Some(ReviewerTrouble::CliFailed)
  );
  assert_eq!(
    ReviewerTrouble::of(ReviewFailure::CouldNotRun, &ReviewerAccount::default(), at(60)),
    Some(ReviewerTrouble::CliFailed),
    "a reviewer whose account Orchester does not know"
  );
  assert_eq!(
    ReviewerTrouble::of(ReviewFailure::TimedOut, &account, at(60)),
    Some(ReviewerTrouble::TimedOut)
  );
  assert_eq!(
    ReviewerTrouble::of(ReviewFailure::Unreadable, &account, at(60)),
    Some(ReviewerTrouble::Unreadable)
  );
  assert_eq!(ReviewerTrouble::of(ReviewFailure::NoReviewer, &account, at(60)), None);
  assert_eq!(account.trouble(at(60)), None, "the account itself can run");
}

#[test]
fn reviewer_trouble_an_account_at_its_limit_says_so_with_its_reset() {
  let account = signed_in(Some(reading(100.0, LimitState::Reached, 3_600)));
  let limit = Some(ReviewerTrouble::PlanLimit {
    resets: Some(at(3_600)),
  });
  assert_eq!(ReviewerTrouble::of(ReviewFailure::CouldNotRun, &account, at(60)), limit);
  assert_eq!(account.trouble(at(60)), limit);
  // A timeout is not the limit's doing: the CLI at its limit refuses at once.
  assert_eq!(
    ReviewerTrouble::of(ReviewFailure::TimedOut, &account, at(60)),
    Some(ReviewerTrouble::TimedOut)
  );
  // Once the window reset, the reading no longer holds the reviewer back.
  assert_eq!(account.trouble(at(3_601)), None);
  assert_eq!(
    ReviewerTrouble::of(ReviewFailure::CouldNotRun, &account, at(3_601)),
    Some(ReviewerTrouble::CliFailed)
  );
  // A limit the CLI reported without its reset says so without one.
  let mut unknown = reading(100.0, LimitState::Reached, 3_600);
  unknown.limits.windows = vec![LimitWindow::new(WEEK, Some(100.0), None, LimitState::Reached, false).expect("window")];
  assert_eq!(
    signed_in(Some(unknown)).trouble(at(60)),
    Some(ReviewerTrouble::PlanLimit { resets: None })
  );
}

#[test]
fn reviewer_trouble_a_signed_out_account_says_nobody_is_signed_in() {
  for state in [AccountState::NotSignedIn, AccountState::Expired] {
    let account = ReviewerAccount {
      state: Some(state),
      reading: None,
    };
    assert_eq!(
      ReviewerTrouble::of(ReviewFailure::CouldNotRun, &account, at(0)),
      Some(ReviewerTrouble::NotSignedIn),
      "{state:?}"
    );
  }
  // The plan limit comes first: signing in again would not lift it.
  let both = ReviewerAccount {
    state: Some(AccountState::Expired),
    reading: Some(reading(100.0, LimitState::Reached, 3_600)),
  };
  assert_eq!(
    both.trouble(at(0)),
    Some(ReviewerTrouble::PlanLimit {
      resets: Some(at(3_600))
    })
  );
}

#[test]
fn reviewer_trouble_health_reads_the_account_and_the_last_review_and_estimate() {
  let fine = ReviewerHealth {
    account: signed_in(None),
    last_review: None,
    last_estimate: None,
  };
  assert_eq!((fine.reviews(at(0)), fine.estimates(at(0))), (None, None));
  let failing = ReviewerHealth {
    last_review: Some(ReviewFailure::CouldNotRun),
    last_estimate: Some(ReviewFailure::CouldNotRun),
    ..fine.clone()
  };
  assert_eq!(failing.reviews(at(0)), Some(ReviewerTrouble::CliFailed));
  assert_eq!(failing.estimates(at(0)), Some(ReviewerTrouble::CliFailed));
  // An account at its limit is said under the Reviewer row before any review fails; the estimates only once one has.
  let limited = ReviewerHealth {
    account: signed_in(Some(reading(100.0, LimitState::Reached, 3_600))),
    ..fine
  };
  let limit = Some(ReviewerTrouble::PlanLimit {
    resets: Some(at(3_600)),
  });
  assert_eq!(limited.reviews(at(0)), limit);
  assert_eq!(limited.estimates(at(0)), None);
  assert_eq!(limited.request(ReviewFailure::CouldNotRun, at(0)), limit);
  assert_eq!(ReviewerTrouble::CliFailed.as_str(), "cli_failed");
}
