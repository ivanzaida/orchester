use std::{collections::HashMap, hash::Hash};

use parking_lot::Mutex;

use crate::error::CloudError;

/// At most `limit` events per key in a sliding window of `window_ms`. In memory, as S1a's limiters: the cloud runs as
/// one instance in v1, and a restart clears the counts (`docs/architecture/server.md`, "Limiters in v1").
pub(crate) struct Quota<K> {
  limit: usize,
  window_ms: i64,
  state: Mutex<State<K>>,
}

struct State<K> {
  events: HashMap<K, Vec<i64>>,
  pruned_at: i64,
}

impl<K: Hash + Eq> Quota<K> {
  pub fn new(limit: usize, window_ms: i64) -> Self {
    Self {
      limit,
      window_ms,
      state: Mutex::new(State {
        events: HashMap::new(),
        pruned_at: i64::MIN,
      }),
    }
  }

  /// Refuses when the key used its quota; otherwise counts this event.
  pub fn take(&self, key: K, now: i64) -> Result<(), CloudError> {
    let mut state = self.state.lock();
    self.prune(&mut state, now);
    let events = state.events.entry(key).or_default();
    self.refuse(events, now)?;
    events.push(now);
    Ok(())
  }

  /// Refuses when the key used its quota, without counting anything: for limits on failures.
  pub fn check(&self, key: &K, now: i64) -> Result<(), CloudError> {
    let mut state = self.state.lock();
    self.prune(&mut state, now);
    match state.events.get_mut(key) {
      Some(events) => self.refuse(events, now),
      None => Ok(()),
    }
  }

  pub fn record(&self, key: K, now: i64) {
    self.state.lock().events.entry(key).or_default().push(now);
  }

  fn refuse(&self, events: &mut Vec<i64>, now: i64) -> Result<(), CloudError> {
    events.retain(|at| *at > now - self.window_ms);
    if events.len() < self.limit {
      return Ok(());
    }
    let oldest = events.iter().min().copied().unwrap_or(now);
    let retry_ms = (oldest + self.window_ms - now).max(1000);
    Err(CloudError::RateLimited(((retry_ms + 999) / 1000).unsigned_abs()))
  }

  fn prune(&self, state: &mut State<K>, now: i64) {
    // Dropping idle keys once a minute keeps the map to the keys seen in the last window.
    if now.saturating_sub(state.pruned_at) < 60_000 {
      return;
    }
    state.pruned_at = now;
    let window = self.window_ms;
    state
      .events
      .retain(|_, events| events.iter().any(|at| *at > now - window));
  }
}
