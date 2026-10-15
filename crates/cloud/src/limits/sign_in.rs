//! S1a's sign-in and password limiters, moved to the cloud (`docs/architecture/server.md`, "Rate limits of the
//! cloud"): 5 failures for an email (or an account's password change) in 15 minutes block it for 15 minutes, 20
//! failures from one address block that address, and 100 from one IPv6 /48 block that network (review 2, N2: a
//! client holds a whole /48 as easily as a /64). A place is reserved before the hashing pool, so parallel guesses
//! cannot pass the limit while their hashes run.
//!
//! The maps are bounded (`Bounded`): idle keys are pruned once a minute, and a full map evicts its oldest key with
//! no state (no attempt in flight, block or failure in the window). A flood of new emails therefore never lifts a
//! block or resets a count; when every key has state the map grows past its bound until pruning shrinks it, and the
//! address and network limits keep bounding how fast a flood adds state.

use std::{net::IpAddr, sync::Arc};

use orchester_core::id::CloudAccountId;
use parking_lot::Mutex;

use super::bounded::Bounded;
use crate::{
  addresses::{rate_address, rate_network},
  error::CloudError,
};

const WINDOW_MS: i64 = 15 * 60 * 1000;
const PRUNE_MS: i64 = 60 * 1000;
const EMAIL_LIMIT: usize = 5;
const ADDRESS_LIMIT: usize = 20;
const NETWORK_LIMIT: usize = 100;
const ACCOUNT_LIMIT: usize = 5;

#[derive(Default)]
struct Failures {
  times: Vec<i64>,
  blocked_until: i64,
  pending: usize,
}

impl Failures {
  fn retry(&self, now: i64) -> Option<u64> {
    (self.blocked_until > now).then(|| (self.blocked_until - now + 999).unsigned_abs() / 1000)
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

  /// Only an entry with no state may be evicted: no attempt in flight (it completes on the entry), no block and no
  /// failure inside the window. Evicting any other would lift a block or reset a count early (review 3).
  fn evictable(&self, now: i64) -> bool {
    !self.retain(now)
  }
}

/// What one attempt counts against.
#[derive(Clone)]
enum Key {
  Email(String),
  Address(IpAddr),
  Network(IpAddr),
  Account(CloudAccountId),
}

struct Counters {
  emails: Bounded<String, Failures>,
  addresses: Bounded<IpAddr, Failures>,
  networks: Bounded<IpAddr, Failures>,
  accounts: Bounded<CloudAccountId, Failures>,
  pruned_at: i64,
}

impl Counters {
  fn prune(&mut self, now: i64) {
    if now.saturating_sub(self.pruned_at) < PRUNE_MS {
      return;
    }
    self.pruned_at = now;
    self.emails.retain(|failures| failures.retain(now));
    self.addresses.retain(|failures| failures.retain(now));
    self.networks.retain(|failures| failures.retain(now));
    self.accounts.retain(|failures| failures.retain(now));
  }

  fn get(&self, key: &Key) -> (Option<&Failures>, usize) {
    match key {
      Key::Email(email) => (self.emails.get(email), EMAIL_LIMIT),
      Key::Address(address) => (self.addresses.get(address), ADDRESS_LIMIT),
      Key::Network(network) => (self.networks.get(network), NETWORK_LIMIT),
      Key::Account(account) => (self.accounts.get(account), ACCOUNT_LIMIT),
    }
  }

  fn reserve(&mut self, key: Key, now: i64) {
    let evictable = |failures: &Failures| failures.evictable(now);
    let failures = match key {
      Key::Email(email) => self.emails.entry(email, evictable),
      Key::Address(address) => self.addresses.entry(address, evictable),
      Key::Network(network) => self.networks.entry(network, evictable),
      Key::Account(account) => self.accounts.entry(account, evictable),
    };
    failures.pending += 1;
  }

  fn complete(&mut self, key: &Key, failed: Option<i64>) {
    let (failures, limit) = match key {
      Key::Email(email) => (self.emails.get_mut(email), EMAIL_LIMIT),
      Key::Address(address) => (self.addresses.get_mut(address), ADDRESS_LIMIT),
      Key::Network(network) => (self.networks.get_mut(network), NETWORK_LIMIT),
      Key::Account(account) => (self.accounts.get_mut(account), ACCOUNT_LIMIT),
    };
    failures
      .expect("an entry with a pending reservation is neither pruned nor evicted")
      .complete(failed, limit);
  }
}

pub struct SignInLimit(Arc<Mutex<Counters>>);

/// A place reserved for one sign-in or password check; it ends as `succeeded` or `failed`.
pub struct Attempt {
  counters: Arc<Mutex<Counters>>,
  keys: Vec<Key>,
  started_at: Option<i64>,
}

impl SignInLimit {
  /// Each of its maps holds at most `max_keys` emails, addresses, networks or accounts.
  pub fn new(max_keys: usize) -> Self {
    Self(Arc::new(Mutex::new(Counters {
      emails: Bounded::new(max_keys),
      addresses: Bounded::new(max_keys),
      networks: Bounded::new(max_keys),
      accounts: Bounded::new(max_keys),
      pruned_at: i64::MIN,
    })))
  }

  pub fn sign_in(&self, email: String, address: IpAddr, now: i64) -> Result<Attempt, CloudError> {
    let mut keys = vec![Key::Email(email), Key::Address(rate_address(address))];
    keys.extend(rate_network(address).map(Key::Network));
    self.reserve(keys, now)
  }

  pub fn password(&self, account: CloudAccountId, now: i64) -> Result<Attempt, CloudError> {
    self.reserve(vec![Key::Account(account)], now)
  }

  fn reserve(&self, keys: Vec<Key>, now: i64) -> Result<Attempt, CloudError> {
    let mut counters = self.0.lock();
    counters.prune(now);
    let retry = keys
      .iter()
      .filter_map(|key| match counters.get(key) {
        (Some(failures), limit) => failures.capacity(now, limit),
        (None, _) => None,
      })
      .max();
    if let Some(retry) = retry {
      return Err(CloudError::RateLimited(retry));
    }
    for key in &keys {
      counters.reserve(key.clone(), now);
    }
    Ok(Attempt {
      counters: self.0.clone(),
      keys,
      started_at: None,
    })
  }
}

impl Attempt {
  pub fn start(&mut self, now: i64) -> Result<(), CloudError> {
    let counters = self.counters.lock();
    let retry = self
      .keys
      .iter()
      .filter_map(|key| counters.get(key).0.and_then(|failures| failures.retry(now)))
      .max();
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
    let keys = std::mem::take(&mut self.keys);
    let mut counters = self.counters.lock();
    for key in &keys {
      counters.complete(key, failed);
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
