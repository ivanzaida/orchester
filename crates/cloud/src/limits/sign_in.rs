//! S1a's sign-in and password limiters, moved to the cloud (`docs/architecture/server.md`, "Rate limits of the
//! cloud"): 5 failures for an email (or an account's password change) in 15 minutes block it for 15 minutes, 20
//! failures from one address block that address. A place is reserved before the hashing pool, so parallel guesses
//! cannot pass the limit while their hashes run.

use std::{collections::HashMap, net::IpAddr, sync::Arc};

use orchester_core::id::CloudAccountId;
use parking_lot::Mutex;

use crate::{addresses::rate_address, error::CloudError};

const WINDOW_MS: i64 = 15 * 60 * 1000;

#[derive(Default)]
struct Failures {
  times: Vec<i64>,
  blocked_until: i64,
  pending: usize,
}

impl Failures {
  fn retry(&self, now: i64) -> Option<u64> {
    (self.blocked_until > now).then(|| ((self.blocked_until - now + 999) / 1000) as u64)
  }

  fn capacity(&self, now: i64, limit: usize) -> Option<u64> {
    self.retry(now).or_else(|| {
      // Reservations may succeed and release capacity shortly; a real failure block uses its full deadline.
      (self.pending + self.times.iter().filter(|at| **at > now - WINDOW_MS).count() >= limit).then_some(1)
    })
  }

  fn complete(&mut self, failed: Option<i64>, limit: usize) {
    self.pending -= 1;
    if let Some(now) = failed {
      self.times.retain(|at| *at > now - WINDOW_MS);
      self.times.push(now);
      if self.times.len() >= limit {
        self.blocked_until = now + WINDOW_MS;
      }
    }
  }

  fn retain(&self, now: i64) -> bool {
    self.pending > 0 || self.blocked_until > now || self.times.iter().any(|at| *at > now - WINDOW_MS)
  }
}

#[derive(Default)]
struct Counters {
  emails: HashMap<String, Failures>,
  addresses: HashMap<IpAddr, Failures>,
  password_accounts: HashMap<CloudAccountId, Failures>,
}

#[derive(Default)]
pub(crate) struct SignInLimit(Arc<Mutex<Counters>>);

enum AttemptKeys {
  SignIn { email: String, address: IpAddr },
  PasswordChange(CloudAccountId),
}

pub(crate) struct Attempt {
  counters: Arc<Mutex<Counters>>,
  keys: Option<AttemptKeys>,
  started_at: Option<i64>,
}

impl SignInLimit {
  pub fn sign_in(&self, email: String, address: IpAddr, now: i64) -> Result<Attempt, CloudError> {
    let address = rate_address(address);
    let mut counters = self.0.lock();
    counters.emails.retain(|_, failures| failures.retain(now));
    counters.addresses.retain(|_, failures| failures.retain(now));
    let email_retry = counters
      .emails
      .get(&email)
      .and_then(|failures| failures.capacity(now, 5));
    let address_retry = counters
      .addresses
      .get(&address)
      .and_then(|failures| failures.capacity(now, 20));
    if let Some(retry) = email_retry.into_iter().chain(address_retry).max() {
      return Err(CloudError::RateLimited(retry));
    }
    counters.emails.entry(email.clone()).or_default().pending += 1;
    counters.addresses.entry(address).or_default().pending += 1;
    Ok(Attempt {
      counters: self.0.clone(),
      keys: Some(AttemptKeys::SignIn { email, address }),
      started_at: None,
    })
  }

  pub fn password(&self, account: CloudAccountId, now: i64) -> Result<Attempt, CloudError> {
    let mut counters = self.0.lock();
    counters.password_accounts.retain(|_, failures| failures.retain(now));
    let failures = counters.password_accounts.entry(account).or_default();
    if let Some(retry) = failures.capacity(now, 5) {
      return Err(CloudError::RateLimited(retry));
    }
    failures.pending += 1;
    Ok(Attempt {
      counters: self.0.clone(),
      keys: Some(AttemptKeys::PasswordChange(account)),
      started_at: None,
    })
  }
}

impl Attempt {
  pub fn start(&mut self, now: i64) -> Result<(), CloudError> {
    let counters = self.counters.lock();
    let retry = match self.keys.as_ref().expect("active reservation starts once") {
      AttemptKeys::SignIn { email, address } => {
        let email = counters.emails.get(email).and_then(|failures| failures.retry(now));
        let address = counters.addresses.get(address).and_then(|failures| failures.retry(now));
        email.into_iter().chain(address).max()
      }
      AttemptKeys::PasswordChange(account) => counters
        .password_accounts
        .get(account)
        .and_then(|failures| failures.retry(now)),
    };
    if let Some(retry) = retry {
      return Err(CloudError::RateLimited(retry));
    }
    self.started_at = Some(now);
    Ok(())
  }

  pub fn succeeded(mut self) {
    self.complete(None);
  }

  pub fn failed(mut self, now: i64) {
    self.complete(Some(now));
  }

  fn complete(&mut self, failed: Option<i64>) {
    let Some(keys) = self.keys.take() else {
      return;
    };
    let mut counters = self.counters.lock();
    match keys {
      AttemptKeys::SignIn { email, address } => {
        counters
          .emails
          .get_mut(&email)
          .expect("pending email reservations are retained")
          .complete(failed, 5);
        counters
          .addresses
          .get_mut(&address)
          .expect("pending address reservations are retained")
          .complete(failed, 20);
      }
      AttemptKeys::PasswordChange(account) => {
        counters
          .password_accounts
          .get_mut(&account)
          .expect("pending password reservations are retained")
          .complete(failed, 5);
      }
    }
  }
}

impl Drop for Attempt {
  fn drop(&mut self) {
    // Cancelling before hashing releases capacity. Once hashing starts, abandoning its result consumes an attempt,
    // otherwise a client could evade the limit by disconnecting while the blocking worker continues.
    self.complete(self.started_at);
  }
}
