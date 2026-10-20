use super::{Blocked, LimitBlock, LimitReading, LimitState, LimitWindow, ReadingSource};
use crate::{connection::AgentProvider, id::AgentAccountId, time::Timestamp};

/// Session-only newest readings, one per account: limits belong to an account and are shared by every project that
/// uses it. Each keeps where its newest report came from (a turn, or a read).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanReadings(Vec<Entry>);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
  reading: LimitReading,
  source: ReadingSource,
}

impl PlanReadings {
  /// The newest reading of any account of `provider`'s CLI, for a view with one entry per CLI.
  pub fn get(&self, provider: AgentProvider) -> Option<&LimitReading> {
    self
      .all()
      .filter(|reading| reading.provider == provider)
      .max_by_key(|reading| reading.at)
  }

  /// The newest reading of `account`.
  pub fn of_account(&self, account: AgentAccountId) -> Option<&LimitReading> {
    self.entry(account).map(|entry| &entry.reading)
  }

  /// Where the newest report of `account` came from.
  pub fn source_of(&self, account: AgentAccountId) -> Option<ReadingSource> {
    self.entry(account).map(|entry| entry.source)
  }

  fn entry(&self, account: AgentAccountId) -> Option<&Entry> {
    self.0.iter().find(|entry| entry.reading.account_id() == Some(account))
  }

  /// Every account's newest reading.
  pub fn all(&self) -> impl Iterator<Item = &LimitReading> {
    self.0.iter().map(|entry| &entry.reading)
  }

  /// Takes in a newer reading an agent turn reported; `false` when it is older or changes nothing.
  pub fn observe(&mut self, reading: LimitReading) -> bool {
    self.observe_from(reading, ReadingSource::Turn)
  }

  /// Takes in a newer reading from `source`; `false` when it is older or changes nothing. A read merges with what
  /// turns reported as a turn's report does: both are the CLI's own words, and the newest wins.
  pub fn observe_from(&mut self, mut reading: LimitReading, source: ReadingSource) -> bool {
    for window in &mut reading.limits.windows {
      window.at = Some(reading.at);
    }
    let account = reading.account_id();
    let Some(entry) = self
      .0
      .iter_mut()
      .find(|old| old.reading.provider == reading.provider && old.reading.account_id() == account)
    else {
      self.0.push(Entry { reading, source });
      return true;
    };
    let old = &mut entry.reading;
    if old.at > reading.at {
      return false;
    }
    let reports_windows = !reading.limits.windows.is_empty();
    for window in &mut reading.limits.windows {
      if let Some(known) = old.limits.windows.iter().find(|known| known.minutes == window.minutes)
        && keeps_known_use(known, window, reading.at)
      {
        *window = known.clone();
      }
    }
    // Both CLIs report sparsely: Claude Code one window per event, Codex App Server's update
    // leaves out what it does not know, which does not clear what was read before.
    // A refusal that names no window keeps only the windows read before that agree with it:
    // those that were reached. Any other would read as current beside a refusal it contradicts.
    let refused_without_window = !reports_windows
      && reading.limits.blocked
        == Some(Blocked {
          reason: LimitBlock::Window,
          window: None,
        });
    for window in &old.limits.windows {
      let agrees = !refused_without_window || window.state(reading.at) == LimitState::Reached;
      if agrees && !reading.limits.windows.iter().any(|new| new.minutes == window.minutes) {
        reading.limits.windows.push(window.clone());
      }
    }
    if reading.limits.plan.is_none() {
      reading.limits.plan.clone_from(&old.limits.plan);
    }
    if reading.limits.credits.is_none() {
      reading.limits.credits.clone_from(&old.limits.credits);
    }
    // An update with windows and no block says the CLI no longer refuses; one without windows
    // (credits or the plan only) says nothing about it. A block is never moved onto a window the
    // update did not name.
    if reading.limits.blocked.is_none() && !reports_windows {
      reading.limits.blocked = old.limits.blocked;
    }
    if old == &reading {
      return false;
    }
    *old = reading;
    entry.source = source;
    true
  }
}

/// Whether a window reported without its use keeps the use read before, with that reading's own
/// age: only while the earlier reading still stands for the same period and says what the new
/// report says (same reset, extra usage and state). A report that disagrees replaces it, and the
/// use reads unknown rather than a value the CLI has contradicted.
fn keeps_known_use(known: &LimitWindow, new: &LimitWindow, at: Timestamp) -> bool {
  new.used.is_none()
    && known.used.is_some()
    && !known.expired(at)
    && new.resets.is_none_or(|resets| known.resets == Some(resets))
    && known.extra == new.extra
    && known.state(at) == new.state(at)
}
