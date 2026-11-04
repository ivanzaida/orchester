//! How full an agent session's context window is after its latest model response (ORC-304): the tokens that response
//! took in, against the window. The window is the one the CLI or the provider reported; only where nothing reported
//! one does [`estimated_window`]'s table of model limits give it, marked as an estimate. A session whose CLI reported
//! no tokens has no reading at all (`None` where one is kept), never an empty one.

use serde::{Deserialize, Serialize};

/// The context window of Claude models (Claude Code's own default).
const CLAUDE_WINDOW: u64 = 200_000;
/// The context window of a Claude model chosen with Claude Code's `[1m]` suffix.
const CLAUDE_LONG_WINDOW: u64 = 1_000_000;
/// Claude Code's model aliases, which name a Claude model of [`CLAUDE_WINDOW`].
const CLAUDE_ALIASES: [&str; 4] = ["opus", "sonnet", "haiku", "opusplan"];

/// One reading: `used` tokens of a `window` that is `None` while nobody reported one and the table knows no limit
/// for the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextUse {
  used: u64,
  window: Option<u64>,
  /// The window comes from the table, not from what the CLI or provider reported.
  estimated: bool,
}

impl ContextUse {
  /// `used` tokens of a `window` the CLI or provider reported.
  pub const fn reported(used: u64, window: u64) -> Self {
    Self {
      used,
      window: Some(window),
      estimated: false,
    }
  }

  /// `used` tokens of a window nobody reported, for a session whose model is not known either.
  pub const fn unknown_window(used: u64) -> Self {
    Self {
      used,
      window: None,
      estimated: false,
    }
  }

  /// `used` tokens of a session nothing reported the window of: the table's limit for `model`, as an estimate, or no
  /// window when the table knows none.
  pub fn of_model(used: u64, model: &str) -> Self {
    let window = estimated_window(model);
    Self {
      used,
      window,
      estimated: window.is_some(),
    }
  }

  /// `used` tokens of a `window` that is the table's estimate when `estimated`.
  pub const fn new(used: u64, window: u64, estimated: bool) -> Self {
    Self {
      used,
      window: Some(window),
      estimated,
    }
  }

  /// Tokens the latest model response took in.
  pub const fn used(&self) -> u64 {
    self.used
  }

  /// The context window in tokens; `None` when it is not known.
  pub const fn window(&self) -> Option<u64> {
    self.window
  }

  /// Whether the window is the table's estimate rather than what the CLI or provider reported.
  pub const fn estimated(&self) -> bool {
    self.estimated
  }

  /// The whole percent of the window `used` takes, rounded down so a share never reads fuller than it is; `None`
  /// while the window is not known. Past the window it says more than 100.
  pub const fn share(&self) -> Option<u64> {
    match self.window {
      Some(0) => Some(100),
      Some(window) => Some(self.used.saturating_mul(100) / window),
      None => None,
    }
  }

  /// How full the window is, as the context meter shows it (ORC-305).
  pub const fn fullness(&self) -> Fullness {
    match (self.share(), self.window) {
      (Some(_), Some(window)) if self.used >= window => Fullness::Full,
      (Some(share), _) if share >= NEAR_FULL_PERCENT => Fullness::NearFull,
      (Some(_), _) => Fullness::Plenty,
      (None, _) => Fullness::Unknown,
    }
  }
}

/// The share from which a window is near full: where a native session compacts on its own.
pub const NEAR_FULL_PERCENT: u64 = 80;

/// How full a session's context window is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fullness {
  /// Under [`NEAR_FULL_PERCENT`].
  Plenty,
  /// From [`NEAR_FULL_PERCENT`], short of the whole window.
  NearFull,
  /// The whole window, or more.
  Full,
  /// Nobody reported the window, so there is no share.
  Unknown,
}

/// The context window of `model` as the table of model limits has it, for a session whose CLI reports none; `None`
/// for a model it does not know.
pub fn estimated_window(model: &str) -> Option<u64> {
  let model = model.trim().to_ascii_lowercase();
  let (name, long) = match model.strip_suffix("[1m]") {
    Some(name) => (name, true),
    None => (model.as_str(), false),
  };
  let claude = name.starts_with("claude-") || CLAUDE_ALIASES.contains(&name);
  match (claude, long) {
    (true, true) => Some(CLAUDE_LONG_WINDOW),
    (true, false) => Some(CLAUDE_WINDOW),
    (false, _) => None,
  }
}
