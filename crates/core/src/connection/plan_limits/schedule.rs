//! When Orchester reads an account's plan limits on its own (`docs/design/agent-accounts.md`, "Plan limits read on
//! their own"): when the app starts, when an account signs in or changes, when the status bar's plan limits hint or
//! the Usage page opens, once after a reset the last reading named passes, and every [`VISIBLE_INTERVAL_MILLIS`]
//! only while the hint or the Usage page is shown. Nothing reads in the background otherwise. A failed read waits
//! before the next try, longer after each failure in a row and longer still when the provider holds requests back;
//! an account nobody is signed in to waits until it changes. Only the times are kept here; the reading itself is in
//! [`super::PlanReadings`].

use super::{LimitReading, read::ReadError};
use crate::{id::AgentAccountId, time::Timestamp};

/// How often the accounts are read while the hint or the Usage page is shown.
pub const VISIBLE_INTERVAL_MILLIS: i64 = 5 * 60 * 1000;
/// The wait after a first failure; each failure in a row doubles it.
pub const RETRY_MILLIS: i64 = 60 * 1000;
/// The wait after the provider held a first read back; each in a row doubles it.
pub const RATE_LIMITED_MILLIS: i64 = 5 * 60 * 1000;
/// The longest wait between two tries.
pub const MAX_WAIT_MILLIS: i64 = 60 * 60 * 1000;

/// What asks for a read of every account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
  /// The app started.
  Start,
  /// An account signed in or changed: also one that was waiting reads again.
  AccountsChanged,
  /// The hint or the Usage page opened.
  Opened,
}

/// How a read ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
  Read,
  Failed,
  /// The provider held the read back.
  RateLimited,
  /// Nobody is signed in to the account.
  SignedOut,
}

impl Outcome {
  pub const fn of(result: Result<(), &ReadError>) -> Self {
    match result {
      Ok(()) => Self::Read,
      Err(ReadError::NotSignedIn) => Self::SignedOut,
      Err(ReadError::RateLimited(_)) => Self::RateLimited,
      Err(_) => Self::Failed,
    }
  }
}

/// The reads of every account Orchester reads on its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadSchedule(Vec<(AgentAccountId, Account)>);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Account {
  /// Asked for by a trigger and not tried since.
  wanted: bool,
  /// When it was last tried, however that ended.
  tried: Option<Timestamp>,
  /// Failures in a row, and which kind the last one was.
  failures: u32,
  rate_limited: bool,
  signed_out: bool,
}

impl Account {
  /// When the next try may start: at once (the epoch, long past), after the wait its failures earned, or, signed out,
  /// not until it changes.
  fn allowed_at(&self) -> Option<Timestamp> {
    if self.signed_out {
      return None;
    }
    let (Some(tried), true) = (self.tried, self.failures > 0) else {
      return Some(Timestamp::from_unix_millis(0));
    };
    let base = if self.rate_limited {
      RATE_LIMITED_MILLIS
    } else {
      RETRY_MILLIS
    };
    let doubled = base.saturating_mul(1_i64 << (self.failures - 1).min(16));
    Some(Timestamp::from_unix_millis(
      tried.unix_millis().saturating_add(doubled.min(MAX_WAIT_MILLIS)),
    ))
  }

  /// When something other than a trigger makes it due: the interval while shown, or the first reset `reading` named
  /// that passes after the last try. `None` when nothing will.
  fn due_at(&self, shown: bool, reading: Option<&LimitReading>) -> Option<Timestamp> {
    let tried = self.tried?;
    let periodic =
      shown.then(|| Timestamp::from_unix_millis(tried.unix_millis().saturating_add(VISIBLE_INTERVAL_MILLIS)));
    let reset = reading.and_then(|reading| next_reset(reading, tried));
    [periodic, reset].into_iter().flatten().min()
  }
}

/// The first reset in `reading` after `after`: a window's, as the CLI reported it.
fn next_reset(reading: &LimitReading, after: Timestamp) -> Option<Timestamp> {
  reading
    .limits
    .windows
    .iter()
    .filter_map(super::LimitWindow::resets)
    .filter(|resets| *resets > after)
    .min()
}

impl ReadSchedule {
  /// Keeps the accounts in `accounts`, in that order, and forgets the others; a new one is read at once.
  pub fn track(&mut self, accounts: &[AgentAccountId]) {
    let mut kept = Vec::with_capacity(accounts.len());
    for account in accounts {
      let known = self.0.iter().position(|(id, _)| id == account);
      let state = known.map_or_else(
        || Account {
          wanted: true,
          ..Account::default()
        },
        |index| self.0[index].1.clone(),
      );
      kept.push((*account, state));
    }
    self.0 = kept;
  }

  /// Asks for a read of every account `trigger` concerns.
  pub fn request(&mut self, trigger: Trigger) {
    for (_, account) in &mut self.0 {
      account.wanted = true;
      if trigger == Trigger::AccountsChanged {
        account.failures = 0;
        account.signed_out = false;
      }
    }
  }

  /// The accounts to read at `now`, while the hint or the Usage page is `shown`, with `reading` giving each account's
  /// newest reading.
  pub fn due<'a>(
    &self,
    now: Timestamp,
    shown: bool,
    reading: impl Fn(AgentAccountId) -> Option<&'a LimitReading>,
  ) -> Vec<AgentAccountId> {
    self
      .0
      .iter()
      .filter(|(id, account)| {
        let allowed = account.allowed_at().is_some_and(|at| at <= now);
        let asked = account.wanted || account.due_at(shown, reading(*id)).is_some_and(|at| at <= now);
        allowed && asked
      })
      .map(|(id, _)| *id)
      .collect()
  }

  /// When an account becomes due without a trigger, after `now`: what a reader waits for; `None` when nothing will.
  pub fn next_due<'a>(
    &self,
    now: Timestamp,
    shown: bool,
    reading: impl Fn(AgentAccountId) -> Option<&'a LimitReading>,
  ) -> Option<Timestamp> {
    self
      .0
      .iter()
      .filter_map(|(id, account)| {
        let allowed = account.allowed_at()?;
        let due = if account.wanted {
          now
        } else {
          account.due_at(shown, reading(*id))?
        };
        Some(due.max(allowed))
      })
      .filter(|at| *at > now)
      .min()
  }

  /// Records how the read of `account` that started at `at` ended.
  pub fn record(&mut self, account: AgentAccountId, at: Timestamp, outcome: Outcome) {
    let Some((_, state)) = self.0.iter_mut().find(|(id, _)| *id == account) else {
      return;
    };
    state.wanted = false;
    state.tried = Some(at);
    state.signed_out = outcome == Outcome::SignedOut;
    match outcome {
      Outcome::Read | Outcome::SignedOut => {
        state.failures = 0;
        state.rate_limited = false;
      }
      Outcome::Failed | Outcome::RateLimited => {
        state.failures = state.failures.saturating_add(1);
        state.rate_limited = outcome == Outcome::RateLimited;
      }
    }
  }
}
