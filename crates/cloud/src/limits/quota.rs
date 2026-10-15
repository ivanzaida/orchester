use std::hash::Hash;

use parking_lot::Mutex;

use super::bounded::Bounded;
use crate::error::CloudError;

/// At most `limit` events per key in a sliding window of `window_ms`. In memory, as S1a's limiters: the cloud runs as
/// one instance in v1, and a restart clears the counts (`docs/architecture/server.md`, "Limiters in v1"). At most
/// `max_keys` keys are tracked; a new key beyond them evicts the oldest key with no event in the window (review 2, N2;
/// review 3), so a flood of new keys never resets a count, and the map grows past its bound while every key counts.
pub struct Quota<K> {
  limit: usize,
  window_ms: i64,
  state: Mutex<State<K>>,
}

struct State<K> {
  events: Bounded<K, Vec<i64>>,
  pruned_at: i64,
}

impl<K: Hash + Eq + Clone> Quota<K> {
  pub fn new(limit: usize, window_ms: i64, max_keys: usize) -> Self {
    Self {
      limit,
      window_ms,
      state: Mutex::new(State {
        events: Bounded::new(max_keys),
        pruned_at: i64::MIN,
      }),
    }
  }

  /// Refuses when the key used its quota; otherwise counts this event.
  pub fn take(&self, key: K, now: i64) -> Result<(), CloudError> {
    let mut state = self.state.lock();
    self.prune(&mut state, now);
    let events = state.events.entry(key, self.stateless(now));
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
    let stateless = self.stateless(now);
    self.state.lock().events.entry(key, stateless).push(now);
  }

  /// Takes back one event counted at `at` by `take`: a reservation that turned out not to count.
  pub fn forgive(&self, key: &K, at: i64) {
    if let Some(events) = self.state.lock().events.get_mut(key)
      && let Some(index) = events.iter().position(|event| *event == at)
    {
      events.swap_remove(index);
    }
  }

  fn stateless(&self, now: i64) -> impl Fn(&Vec<i64>) -> bool {
    let window = self.window_ms;
    move |events| events.iter().all(|at| *at <= now - window)
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
    state.events.retain(|events| events.iter().any(|at| *at > now - window));
  }
}
