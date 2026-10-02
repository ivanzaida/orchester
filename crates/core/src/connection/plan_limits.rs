//! Account plan readings reported by an agent turn, never an estimate of current usage.

mod parse;
mod readings;
pub use parse::{claude_limits, codex_limits};
pub use readings::PlanReadings;
use serde::{Deserialize, Serialize};

use crate::{connection::AgentProvider, time::Timestamp};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimitState {
  Normal,
  Near,
  Reached,
}

/// A validated provider window. Hundredths retain fractional percentages without float equality;
/// a CLI may report a window's state without how much of it is used.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "WindowData", into = "WindowData")]
pub struct LimitWindow {
  minutes: u64,
  used: Option<u32>,
  resets: Option<Timestamp>,
  state: LimitState,
  extra: bool,
  at: Option<Timestamp>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WindowData {
  minutes: u64,
  used: Option<u32>,
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
    let percent = data.used.map(|used| f64::from(used) / 100.0);
    let mut window = Self::new(data.minutes, percent, data.resets, data.state, data.extra)?;
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
  /// A window of `minutes`, `percent` used when the CLI says. A CLI that counts past the limit
  /// reports more than 100 %, which reads as the full window.
  pub fn new(
    minutes: u64,
    percent: Option<f64>,
    resets: Option<Timestamp>,
    state: LimitState,
    extra: bool,
  ) -> Result<Self, InvalidWindow> {
    if minutes == 0 || percent.is_some_and(|percent| !percent.is_finite() || percent < 0.0) {
      return Err(InvalidWindow);
    }
    Ok(Self {
      minutes,
      used: percent.map(|percent| (percent.min(100.0) * 100.0).round() as u32),
      resets,
      state,
      extra,
      at: None,
    })
  }
  pub const fn minutes(&self) -> u64 {
    self.minutes
  }
  /// How much of the window is used; `None` when the CLI reported only its state.
  pub fn percent(&self) -> Option<f64> {
    self.used.map(|used| f64::from(used) / 100.0)
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
    let used = self.used.unwrap_or_default();
    if self.state == LimitState::Reached || used >= 10_000 {
      return LimitState::Reached;
    }
    if self.state == LimitState::Near || used >= 8_000 {
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

/// Why a CLI says it refuses new turns (Codex App Server's `rateLimitReachedType`; Claude Code's
/// `rejected` status without its window).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimitBlock {
  /// A usage window is used up; the block lifts when that window resets.
  Window,
  /// The workspace has no credits left; a window reset does not lift it.
  Credits,
  /// The workspace's own usage limit is reached; a window reset does not lift it.
  Workspace,
  /// A reason this version does not know; nothing says when it lifts.
  Other,
}

impl LimitBlock {
  pub const fn lifts_at_reset(self) -> bool {
    matches!(self, Self::Window)
  }
}

/// One CLI report: Claude Code reports one window at a time, Codex App Server the windows it knows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanLimits {
  pub windows: Vec<LimitWindow>,
  pub plan: Option<String>,
  pub credits: Option<Credits>,
  pub blocked: Option<LimitBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitReading {
  pub provider: AgentProvider,
  pub at: Timestamp,
  pub limits: PlanLimits,
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

  /// The account's state: a block that a window reset does not lift, or one with no window to
  /// lift it, holds until a newer reading; otherwise the limiting window decides.
  pub fn state(&self, now: Timestamp) -> LimitState {
    match self.limits.blocked {
      Some(block) if !block.lifts_at_reset() => LimitState::Reached,
      Some(_) if self.limits.windows.is_empty() => LimitState::Reached,
      _ => self
        .limiting(now)
        .map_or(LimitState::Normal, |window| window.state(now)),
    }
  }
}

/// Marks the fullest of `windows` as the one a window block applies to, unless one already is.
fn mark_blocked(windows: &mut [LimitWindow]) {
  if windows.iter().any(|window| window.state == LimitState::Reached) {
    return;
  }
  if let Some(window) = windows.iter_mut().max_by_key(|window| window.used) {
    window.state = LimitState::Reached;
  }
}
