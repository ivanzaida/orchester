//! Limits on failed joins (`docs/architecture/server.md`, "Codes"): 10 failed code redemptions or cloud joins from one
//! client network (an IPv4 address, an IPv6 /48) in 15 minutes block that network for 15 minutes. 200 failed
//! redemptions on the whole server in any hour pause redemption until fewer than 200 remain in the last hour, at least
//! 15 minutes; cloud joins have a server-wide budget of their own, so neither kind can pause the other. Redemptions
//! from this computer (loopback) are exempt from the server-wide budgets, so the person at the host can always redeem
//! the setup code. Attempts in flight count against the budgets, so concurrent guesses cannot overrun them. A restart
//! clears the counters.

use std::{collections::HashMap, net::IpAddr, sync::Arc};

use parking_lot::Mutex;

use crate::{addresses::rate_address, error::ServerError};

const MINUTE_MS: i64 = 60 * 1000;

#[derive(Clone, Copy)]
struct Rule {
  limit: usize,
  window_ms: i64,
  block_ms: i64,
}

const PER_ADDRESS: Rule = Rule {
  limit: 10,
  window_ms: 15 * MINUTE_MS,
  block_ms: 15 * MINUTE_MS,
};
const SERVER_WIDE: Rule = Rule {
  limit: 200,
  window_ms: 60 * MINUTE_MS,
  block_ms: 15 * MINUTE_MS,
};

#[derive(Default)]
struct Failures {
  times: Vec<i64>,
  blocked_until: i64,
  pending: usize,
}

impl Failures {
  /// Seconds to wait before another attempt, when the budget is spent.
  fn retry(&self, rule: Rule, now: i64) -> Option<u64> {
    if self.blocked_until > now {
      return u64::try_from((self.blocked_until - now + 999) / 1000).ok();
    }
    // Attempts in flight may still succeed and give their place back; a real block uses its full deadline.
    let recent = self.times.iter().filter(|at| **at > now - rule.window_ms).count();
    (self.pending + recent >= rule.limit).then_some(1)
  }

  fn complete(&mut self, rule: Rule, failed_at: Option<i64>) {
    self.pending -= 1;
    let Some(at) = failed_at else { return };
    self.times.retain(|time| *time > at - rule.window_ms);
    self.times.push(at);
    if self.times.len() >= rule.limit {
      // The window slides: the block lasts until the oldest failure that filled it ages out, and at least the
      // rule's block.
      self.times.sort_unstable();
      let room_at = self.times[self.times.len() - rule.limit] + rule.window_ms;
      self.blocked_until = self.blocked_until.max(at + rule.block_ms).max(room_at);
    }
  }

  fn active(&self, rule: Rule, now: i64) -> bool {
    self.pending > 0 || self.blocked_until > now || self.times.iter().any(|at| *at > now - rule.window_ms)
  }
}

#[derive(Default)]
struct Counters {
  addresses: HashMap<IpAddr, Failures>,
  redemptions: Failures,
  cloud_joins: Failures,
}

impl Counters {
  fn server_wide(&mut self, kind: JoinKind) -> &mut Failures {
    match kind {
      JoinKind::Code => &mut self.redemptions,
      JoinKind::Cloud => &mut self.cloud_joins,
    }
  }
}

/// What an attempt is counted against.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum JoinKind {
  Code,
  Cloud,
}

#[derive(Default)]
pub(crate) struct JoinLimits(Arc<Mutex<Counters>>);

impl JoinLimits {
  /// Reserves a place for one attempt from `address`, or the seconds to wait.
  pub fn begin(&self, kind: JoinKind, address: IpAddr, now: i64) -> Result<Attempt, ServerError> {
    let server_wide = !address.to_canonical().is_loopback();
    let address = rate_address(address);
    let mut counters = self.0.lock();
    counters
      .addresses
      .retain(|_, failures| failures.active(PER_ADDRESS, now));
    let address_retry = counters
      .addresses
      .get(&address)
      .and_then(|failures| failures.retry(PER_ADDRESS, now));
    let server_retry = server_wide
      .then(|| counters.server_wide(kind).retry(SERVER_WIDE, now))
      .flatten();
    if let Some(retry) = address_retry.into_iter().chain(server_retry).max() {
      return Err(ServerError::RateLimited(retry));
    }
    counters.addresses.entry(address).or_default().pending += 1;
    if server_wide {
      counters.server_wide(kind).pending += 1;
    }
    Ok(Attempt {
      counters: self.0.clone(),
      kind,
      address,
      server_wide,
      started_at: now,
      done: false,
    })
  }
}

/// One reserved attempt. Dropped without an outcome, it counts as failed: a client that disconnects while its code
/// is checked must not get its guess back.
pub(crate) struct Attempt {
  counters: Arc<Mutex<Counters>>,
  kind: JoinKind,
  address: IpAddr,
  server_wide: bool,
  started_at: i64,
  done: bool,
}

impl Attempt {
  pub fn succeeded(mut self) {
    self.complete(None);
  }

  pub fn failed(mut self, now: i64) {
    self.complete(Some(now));
  }

  fn complete(&mut self, failed_at: Option<i64>) {
    if std::mem::replace(&mut self.done, true) {
      return;
    }
    let mut counters = self.counters.lock();
    counters
      .addresses
      .get_mut(&self.address)
      .expect("an address with an attempt in flight is retained")
      .complete(PER_ADDRESS, failed_at);
    if self.server_wide {
      counters.server_wide(self.kind).complete(SERVER_WIDE, failed_at);
    }
  }
}

impl Drop for Attempt {
  fn drop(&mut self) {
    self.complete(Some(self.started_at));
  }
}
