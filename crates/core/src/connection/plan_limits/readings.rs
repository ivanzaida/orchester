use super::{LimitBlock, LimitReading, mark_blocked};
use crate::connection::AgentProvider;

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
    // Both CLIs report sparsely: Claude Code one window per event, Codex App Server's update
    // leaves out what it does not know, which does not clear what was read before.
    for window in &old.limits.windows {
      if !reading.limits.windows.iter().any(|new| new.minutes == window.minutes) {
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
    // (credits or the plan only) says nothing about it.
    if reading.limits.blocked.is_none() && !reports_windows {
      reading.limits.blocked = old.limits.blocked;
    }
    if reading.limits.blocked == Some(LimitBlock::Window) {
      mark_blocked(&mut reading.limits.windows);
    }
    if old == &reading {
      return false;
    }
    *old = reading;
    true
  }
}
