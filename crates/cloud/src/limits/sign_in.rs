//! S1a's sign-in and password limiters, moved to the cloud (`docs/architecture/server.md`, "Rate limits of the
//! cloud"): 5 failures for an email (or an account's password change) in 15 minutes block it for 15 minutes, 20
//! failures from one address block that address. A place is reserved before the hashing pool, so parallel guesses
//! cannot pass the limit while their hashes run.
//!
//! The maps are bounded (review 1, M6): idle keys are pruned once a minute, not on every sign-in, and while a map
//! holds `max_keys` keys a new key is refused with `503 busy` rather than grow it; keys already counted go on.

use std::{collections::HashMap, net::IpAddr, sync::Arc};

use orchester_api::cloud::CloudErrorCode;
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

struct Counters {
  emails: HashMap<String, Failures>,
  addresses: HashMap<IpAddr, Failures>,
  password_accounts: HashMap<CloudAccountId, Failures>,
  max_keys: usize,
  pruned_at: i64,
}

impl Counters {
  fn prune(&mut self, now: i64) {
    if now.saturating_sub(self.pruned_at) < PRUNE_MS {
      return;
    }
    self.pruned_at = now;
    self.emails.retain(|_, failures| failures.retain(now));
    self.addresses.retain(|_, failures| failures.retain(now));
    self.password_accounts.retain(|_, failures| failures.retain(now));
  }
}

const PRUNE_MS: i64 = 60 * 1000;

/// Whether `key` is counted already or the map has room for it.
fn room<K: std::hash::Hash + Eq, V>(map: &HashMap<K, V>, key: &K, max_keys: usize) -> Result<(), CloudError> {
  if map.contains_key(key) || map.len() < max_keys {
    Ok(())
  } else {
    Err(CloudErrorCode::Busy.into())
  }
}

pub struct SignInLimit(Arc<Mutex<Counters>>);

enum AttemptKeys {
  SignIn { email: String, address: IpAddr },
  PasswordChange(CloudAccountId),
}

/// A place reserved for one sign-in or password check; it ends as `succeeded` or `failed`.
pub struct Attempt {
  counters: Arc<Mutex<Counters>>,
  keys: Option<AttemptKeys>,
  started_at: Option<i64>,
}

impl SignInLimit {
  /// Each of its maps holds at most `max_keys` emails, addresses or accounts.
  pub fn new(max_keys: usize) -> Self {
    Self(Arc::new(Mutex::new(Counters {
      emails: HashMap::new(),
      addresses: HashMap::new(),
      password_accounts: HashMap::new(),
      max_keys,
      pruned_at: i64::MIN,
    })))
  }

  pub fn sign_in(&self, email: String, address: IpAddr, now: i64) -> Result<Attempt, CloudError> {
    let address = rate_address(address);
    let mut counters = self.0.lock();
    counters.prune(now);
    room(&counters.emails, &email, counters.max_keys)?;
    room(&counters.addresses, &address, counters.max_keys)?;
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
    counters.prune(now);
    room(&counters.password_accounts, &account, counters.max_keys)?;
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
