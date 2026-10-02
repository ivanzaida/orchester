use super::{Blocked, LimitBlock, LimitReading, LimitState, LimitWindow};
use crate::{connection::AgentProvider, time::Timestamp};

/// Session-only newest readings, one account per CLI on this computer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanReadings(Vec<LimitReading>);

impl PlanReadings {
  pub fn get(&self, provider: AgentProvider) -> Option<&LimitReading> {
    self.0.iter().find(|reading| reading.provider == provider)
  }

  /// Takes in a newer reading; `false` when it is older or changes nothing.
  pub fn observe(&mut self, mut reading: LimitReading) -> bool {
    for window in &mut reading.limits.windows {
      window.at = Some(reading.at);
    }
    let Some(old) = self.0.iter_mut().find(|old| old.provider == reading.provider) else {
      self.0.push(reading);
      return true;
    };
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
