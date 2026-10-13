//! Account plan readings reported by an agent turn, never an estimate of current usage.

mod parse;
mod readings;
pub use parse::{claude_limits, codex_limits};
pub use readings::PlanReadings;
use serde::{Deserialize, Serialize};

use crate::{connection::AgentProvider, id::AgentAccountId, time::Timestamp};

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

/// A CLI's refusal as it reported it. Kept apart from the windows: a window's own state is only
/// what the CLI said about that window, never a block inferred for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocked {
  pub reason: LimitBlock,
  /// The window (its minutes) a [`LimitBlock::Window`] applies to: the fullest window of the same
  /// update. `None` when the update named no window, so nothing says when the block lifts.
  pub window: Option<u64>,
}

/// One CLI report: Claude Code reports one window at a time, Codex App Server the windows it knows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanLimits {
  pub windows: Vec<LimitWindow>,
  pub plan: Option<String>,
  pub credits: Option<Credits>,
  pub blocked: Option<Blocked>,
}

/// What one account's CLI reported of its plan limits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitReading {
  pub provider: AgentProvider,
  /// The account the CLI ran on (`docs/design/agent-accounts.md`); `None` for a reading from before accounts, which
  /// was Default's.
  #[serde(default)]
  pub account: Option<AgentAccountId>,
  pub at: Timestamp,
  pub limits: PlanLimits,
}

impl LimitReading {
  /// The account the reading belongs to: Default's for one that names none.
  pub fn account_id(&self) -> Option<AgentAccountId> {
    self
      .account
      .or_else(|| crate::account::AccountCli::of(self.provider).map(crate::account::AccountCli::default_account))
  }

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
        let priority = match self.window_state(window, now) {
          LimitState::Normal => 0,
          LimitState::Near => 1,
          LimitState::Reached => 2,
        };
        (priority, window.used)
      })
  }

  /// `window`'s state, reached also while the window block of the update that named it holds.
  pub fn window_state(&self, window: &LimitWindow, now: Timestamp) -> LimitState {
    let blocked = self
      .limits
      .blocked
      .is_some_and(|blocked| blocked.reason == LimitBlock::Window && blocked.window == Some(window.minutes));
    if blocked && !window.expired(now) {
      return LimitState::Reached;
    }
    window.state(now)
  }

  /// A block that no window reset lifts: its reason does not lift at a reset, or the update named
  /// no window. It holds until a newer reading.
  pub fn block_beyond_reset(&self) -> Option<LimitBlock> {
    let blocked = self.limits.blocked?;
    let window = blocked
      .window
      .filter(|minutes| self.limits.windows.iter().any(|window| window.minutes == *minutes));
    (!blocked.reason.lifts_at_reset() || window.is_none()).then_some(blocked.reason)
  }

  /// When the limit the account reached lifts: the reset of the window it reached; `None` when it reached none, or
  /// the CLI did not say when that window resets.
  pub fn resets_at(&self, now: Timestamp) -> Option<Timestamp> {
    if self.block_beyond_reset().is_some() {
      return None;
    }
    self
      .limiting(now)
      .filter(|window| self.window_state(window, now) == LimitState::Reached)
      .and_then(LimitWindow::resets)
  }

  /// When the limit the account had reached when the CLI reported this lifts, as it said then: also once that time
  /// has passed, so a run stopped at the limit continues when a runner that was not running at the reset starts.
  pub fn reached_limit_resets(&self) -> Option<Timestamp> {
    self.resets_at(self.at)
  }

  /// The account's state: a block no reset lifts holds; otherwise the limiting window decides.
  pub fn state(&self, now: Timestamp) -> LimitState {
    if self.block_beyond_reset().is_some() {
      return LimitState::Reached;
    }
    self
      .limiting(now)
      .map_or(LimitState::Normal, |window| self.window_state(window, now))
  }
}
