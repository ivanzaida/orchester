//! Why auto mode's reviewer could not judge a request, or cannot run now, as Orchester knows it (ORC-611): what the
//! review recorded (it timed out, its answer could not be read, its CLI failed) and, for a CLI that failed, what
//! Orchester knows of the account its connection runs on — at its plan limit, with when it resets, or not signed in.
//! The story-point estimator runs on the same connection, so its failures read the same way.

use super::ReviewFailure;
use crate::{
  account::AccountState,
  connection::plan_limits::{LimitReading, LimitState},
  time::Timestamp,
};

/// Why the reviewer could not run, as the person reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewerTrouble {
  /// Its account reached its plan limit; when the limit lifts, when the CLI said.
  PlanLimit { resets: Option<Timestamp> },
  /// Nobody is signed in to its account, or the sign-in expired.
  NotSignedIn,
  /// Its CLI could not start, or ended with an error.
  CliFailed,
  /// It did not answer in time.
  TimedOut,
  /// Its answer could not be read.
  Unreadable,
}

impl ReviewerTrouble {
  /// Stable name for MCP and the window's element tree.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::PlanLimit { .. } => "plan_limit",
      Self::NotSignedIn => "not_signed_in",
      Self::CliFailed => "cli_failed",
      Self::TimedOut => "timed_out",
      Self::Unreadable => "unreadable",
    }
  }

  /// Why a review or an estimate that ended with `failure` could not run, at `now`: a CLI that failed on an account
  /// that is at its limit or signed out failed for that; `None` for one that never asked a reviewer.
  pub fn of(failure: ReviewFailure, account: &ReviewerAccount, now: Timestamp) -> Option<Self> {
    match failure {
      ReviewFailure::TimedOut => Some(Self::TimedOut),
      ReviewFailure::Unreadable => Some(Self::Unreadable),
      ReviewFailure::CouldNotRun => Some(account.trouble(now).unwrap_or(Self::CliFailed)),
      ReviewFailure::NoReviewer => None,
    }
  }
}

/// What Orchester knows of the account the reviewer's connection runs on; nothing for a native connection or a
/// provider without accounts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewerAccount {
  /// Its sign-in, as its last check found it.
  pub state: Option<AccountState>,
  /// Its newest plan limits reading.
  pub reading: Option<LimitReading>,
}

impl ReviewerAccount {
  /// What keeps the account from running at `now`: its plan limit, while its newest reading says it is reached, then
  /// a sign-in that is missing or expired.
  pub fn trouble(&self, now: Timestamp) -> Option<ReviewerTrouble> {
    if let Some(reading) = self
      .reading
      .as_ref()
      .filter(|reading| reading.state(now) == LimitState::Reached)
    {
      return Some(ReviewerTrouble::PlanLimit {
        resets: reading.resets_at(now),
      });
    }
    matches!(self.state, Some(AccountState::NotSignedIn | AccountState::Expired))
      .then_some(ReviewerTrouble::NotSignedIn)
  }
}

/// The project's reviewer as Project settings › Runs reads it: its account, and how its own last review and the
/// estimator's last request ended (`None` for one that gave its answer, or when none was recorded).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewerHealth {
  pub account: ReviewerAccount,
  pub last_review: Option<ReviewFailure>,
  pub last_estimate: Option<ReviewFailure>,
}

impl ReviewerHealth {
  /// Why the reviewer cannot review at `now`: its account cannot run, or its last review failed.
  pub fn reviews(&self, now: Timestamp) -> Option<ReviewerTrouble> {
    self.account.trouble(now).or_else(|| {
      self
        .last_review
        .and_then(|failure| ReviewerTrouble::of(failure, &self.account, now))
    })
  }

  /// Why the estimator's last request gave no points, read at `now`; `None` when it gave them.
  pub fn estimates(&self, now: Timestamp) -> Option<ReviewerTrouble> {
    self
      .last_estimate
      .and_then(|failure| ReviewerTrouble::of(failure, &self.account, now))
  }

  /// Why a request whose review ended with `failure` could not be judged, read at `now`.
  pub fn request(&self, failure: ReviewFailure, now: Timestamp) -> Option<ReviewerTrouble> {
    ReviewerTrouble::of(failure, &self.account, now)
  }
}
