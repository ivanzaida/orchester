use std::{collections::HashMap, net::IpAddr, sync::Arc};

use orchester_core::id::UserId;
use parking_lot::Mutex;

use crate::{addresses::rate_address, error::ServerError};

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
  usernames: HashMap<String, Failures>,
  addresses: HashMap<IpAddr, Failures>,
  password_users: HashMap<UserId, Failures>,
}

#[derive(Default)]
pub(crate) struct RateLimit(Arc<Mutex<Counters>>);

enum AttemptKeys {
  SignIn { username: String, address: IpAddr },
  PasswordChange(UserId),
}

pub(crate) struct Attempt {
  counters: Arc<Mutex<Counters>>,
  keys: Option<AttemptKeys>,
  started_at: Option<i64>,
}

impl RateLimit {
  pub fn sign_in(&self, username: String, address: IpAddr, now: i64) -> Result<Attempt, ServerError> {
    let address = rate_address(address);
    let mut counters = self.0.lock();
    counters.usernames.retain(|_, failures| failures.retain(now));
    counters.addresses.retain(|_, failures| failures.retain(now));
    let user = counters
      .usernames
      .get(&username)
      .and_then(|failures| failures.capacity(now, 5));
    let address_retry = counters
      .addresses
      .get(&address)
      .and_then(|failures| failures.capacity(now, 20));
    if let Some(retry) = user.into_iter().chain(address_retry).max() {
      return Err(ServerError::RateLimited(retry));
    }
    counters.usernames.entry(username.clone()).or_default().pending += 1;
    counters.addresses.entry(address).or_default().pending += 1;
    Ok(Attempt {
      counters: self.0.clone(),
      keys: Some(AttemptKeys::SignIn { username, address }),
      started_at: None,
    })
  }

  pub fn password(&self, user: UserId, now: i64) -> Result<Attempt, ServerError> {
    let mut counters = self.0.lock();
    counters.password_users.retain(|_, failures| failures.retain(now));
    let failures = counters.password_users.entry(user).or_default();
    if let Some(retry) = failures.capacity(now, 5) {
      return Err(ServerError::RateLimited(retry));
    }
    failures.pending += 1;
    Ok(Attempt {
      counters: self.0.clone(),
      keys: Some(AttemptKeys::PasswordChange(user)),
      started_at: None,
    })
  }
}

impl Attempt {
  pub fn start(&mut self, now: i64) -> Result<(), ServerError> {
    let counters = self.counters.lock();
    let retry = match self.keys.as_ref().expect("active reservation starts once") {
      AttemptKeys::SignIn { username, address } => {
        let user = counters
          .usernames
          .get(username)
          .and_then(|failures| failures.retry(now));
        let address = counters.addresses.get(address).and_then(|failures| failures.retry(now));
        user.into_iter().chain(address).max()
      }
      AttemptKeys::PasswordChange(user) => counters
        .password_users
        .get(user)
        .and_then(|failures| failures.retry(now)),
    };
    if let Some(retry) = retry {
      return Err(ServerError::RateLimited(retry));
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
      AttemptKeys::SignIn { username, address } => {
        counters
          .usernames
          .get_mut(&username)
          .expect("pending username reservations are retained")
          .complete(failed, 5);
        counters
          .addresses
          .get_mut(&address)
          .expect("pending address reservations are retained")
          .complete(failed, 20);
      }
      AttemptKeys::PasswordChange(user) => {
        counters
          .password_users
          .get_mut(&user)
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
