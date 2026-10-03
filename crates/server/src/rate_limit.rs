use std::{collections::HashMap, net::IpAddr};

use orchester_core::id::UserId;
use parking_lot::Mutex;

use crate::addresses::rate_address;

const WINDOW_MS: i64 = 15 * 60 * 1000;

#[derive(Default)]
struct Failures {
  times: Vec<i64>,
  blocked_until: i64,
}

impl Failures {
  fn retry(&self, now: i64) -> Option<u64> {
    (self.blocked_until > now).then(|| ((self.blocked_until - now + 999) / 1000) as u64)
  }

  fn fail(&mut self, now: i64, limit: usize) {
    self.times.retain(|at| *at > now - WINDOW_MS);
    self.times.push(now);
    if self.times.len() >= limit {
      self.blocked_until = now + WINDOW_MS;
    }
  }

  fn retain(&self, now: i64) -> bool {
    self.blocked_until > now || self.times.last().is_some_and(|at| *at > now - WINDOW_MS)
  }
}

#[derive(Default)]
struct Counters {
  usernames: HashMap<String, Failures>,
  addresses: HashMap<IpAddr, Failures>,
  password_users: HashMap<UserId, Failures>,
}

#[derive(Default)]
pub(crate) struct RateLimit(Mutex<Counters>);

impl RateLimit {
  pub fn check(&self, username: &str, address: IpAddr, now: i64) -> Option<u64> {
    let mut counters = self.0.lock();
    counters.usernames.retain(|_, failures| failures.retain(now));
    counters.addresses.retain(|_, failures| failures.retain(now));
    let user = counters
      .usernames
      .get(username)
      .and_then(|failures| failures.retry(now));
    let address = counters
      .addresses
      .get(&rate_address(address))
      .and_then(|failures| failures.retry(now));
    user.into_iter().chain(address).max()
  }

  pub fn failed(&self, username: String, address: IpAddr, now: i64) {
    let mut counters = self.0.lock();
    counters.usernames.entry(username).or_default().fail(now, 5);
    counters
      .addresses
      .entry(rate_address(address))
      .or_default()
      .fail(now, 20);
  }

  pub fn check_password(&self, user: UserId, now: i64) -> Option<u64> {
    let mut counters = self.0.lock();
    counters.password_users.retain(|_, failures| failures.retain(now));
    counters
      .password_users
      .get(&user)
      .and_then(|failures| failures.retry(now))
  }

  pub fn failed_password(&self, user: UserId, now: i64) {
    self.0.lock().password_users.entry(user).or_default().fail(now, 5);
  }
}
