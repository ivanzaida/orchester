//! Account plan readings reported by an agent turn, never an estimate of current usage.

mod parse;
pub use parse::{claude_limits, codex_limits};
use serde::{Deserialize, Serialize};

use crate::{connection::AgentProvider, time::Timestamp};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimitState {
  Normal,
  Near,
  Reached,
}

/// A validated provider window. Hundredths retain fractional percentages without float equality.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "WindowData", into = "WindowData")]
pub struct LimitWindow {
  minutes: u64,
  used: u32,
  resets: Option<Timestamp>,
  state: LimitState,
  extra: bool,
  at: Option<Timestamp>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WindowData {
  minutes: u64,
  used: u32,
  resets: Option<Timestamp>,
  state: LimitState,
  extra: bool,
  #[serde(default)]
  at: Option<Timestamp>,
}

#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid plan window duration or percentage")]
pub struct InvalidWindow;

impl TryFrom<WindowData> for LimitWindow {
  type Error = InvalidWindow;
  fn try_from(data: WindowData) -> Result<Self, Self::Error> {
    let mut window = Self::new(
      data.minutes,
      f64::from(data.used) / 100.0,
      data.resets,
      data.state,
      data.extra,
    )
    .ok_or(InvalidWindow)?;
    window.at = data.at;
    Ok(window)
  }
}

impl From<LimitWindow> for WindowData {
  fn from(window: LimitWindow) -> Self {
    Self {
      minutes: window.minutes,
      used: window.used,
      resets: window.resets,
      state: window.state,
      extra: window.extra,
      at: window.at,
    }
  }
}

impl LimitWindow {
  pub fn new(minutes: u64, percent: f64, resets: Option<Timestamp>, state: LimitState, extra: bool) -> Option<Self> {
    if minutes == 0 || !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
      return None;
    }
    Some(Self {
      minutes,
      used: (percent * 100.0).round() as u32,
      resets,
      state,
      extra,
      at: None,
    })
  }
  pub const fn minutes(&self) -> u64 {
    self.minutes
  }
  pub fn percent(&self) -> f64 {
    f64::from(self.used) / 100.0
  }
  pub const fn resets(&self) -> Option<Timestamp> {
    self.resets
  }
  pub const fn extra(&self) -> bool {
    self.extra
  }
  /// Arrival time of this window, preserved when another Claude window updates.
  pub const fn reported_at(&self) -> Option<Timestamp> {
    self.at
  }
  pub fn expired(&self, now: Timestamp) -> bool {
    self.resets.is_some_and(|at| at <= now)
  }
  pub fn state(&self, now: Timestamp) -> LimitState {
    if self.expired(now) {
      return LimitState::Normal;
    }
    if self.extra {
      return LimitState::Near;
    }
    if self.state == LimitState::Reached || self.used >= 10_000 {
      return LimitState::Reached;
    }
    if self.state == LimitState::Near || self.used >= 8_000 {
      return LimitState::Near;
    }
    LimitState::Normal
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credits {
  pub has_credits: bool,
  pub unlimited: bool,
  pub balance: Option<String>,
}

/// One CLI report; Claude reports one window at a time, Codex replaces both.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanLimits {
  pub windows: Vec<LimitWindow>,
  pub plan: Option<String>,
  pub credits: Option<Credits>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitReading {
  pub provider: AgentProvider,
  pub at: Timestamp,
  pub limits: PlanLimits,
}

/// Session-only newest readings, one account per CLI on this computer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanReadings(Vec<LimitReading>);

impl PlanReadings {
  pub fn get(&self, provider: AgentProvider) -> Option<&LimitReading> {
    self.0.iter().find(|reading| reading.provider == provider)
  }
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
    if reading.provider == AgentProvider::ClaudeCodeCli {
      for window in &old.limits.windows {
        if !reading.limits.windows.iter().any(|new| new.minutes == window.minutes) {
          reading.limits.windows.push(window.clone());
        }
      }
    }
    if old == &reading {
      return false;
    }
    *old = reading;
    true
  }
}

impl LimitReading {
  pub fn fullest(&self, now: Timestamp) -> Option<&LimitWindow> {
    self
      .limits
      .windows
      .iter()
      .filter(|window| !window.expired(now))
      .max_by_key(|window| window.used)
  }

  /// The window responsible for the strongest provider warning or block.
  pub fn limiting(&self, now: Timestamp) -> Option<&LimitWindow> {
    self
      .limits
      .windows
      .iter()
      .filter(|window| !window.expired(now))
      .max_by_key(|window| {
        let priority = match window.state(now) {
          LimitState::Normal => 0,
          LimitState::Near => 1,
          LimitState::Reached => 2,
        };
        (priority, window.used)
      })
  }
}
