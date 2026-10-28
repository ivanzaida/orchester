//! Token accounting (`docs/design/orchester-workspace.md`, "Token budgets"): counts come from
//! the usage each CLI reports, never from estimates. Counted = input + output; cached input is
//! shown but not counted. Codex reports cached tokens as part of input (OpenAI-style usage), so
//! counted input is input − cached.

use std::ops::AddAssign;

use serde::{Deserialize, Serialize};

use crate::tokens::TokensUsed;

/// Usage one CLI invocation or turn reported, or the sum of several.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenUsage {
  /// All input tokens, cached ones included.
  pub input: u64,
  /// The part of `input` read from the provider's cache.
  pub cached_input: u64,
  /// All output tokens, reasoning included.
  pub output: u64,
}

impl TokenUsage {
  /// What counts toward a budget, as [`TokensUsed`].
  pub const fn counted_tokens(self) -> TokensUsed {
    TokensUsed::new(self.counted())
  }

  /// Input that counts toward a budget: input without the cached part.
  pub const fn counted_input(self) -> u64 {
    self.input.saturating_sub(self.cached_input)
  }

  /// Tokens that count toward a budget.
  pub const fn counted(self) -> u64 {
    self.counted_input().saturating_add(self.output)
  }

  /// Whether this, one invocation's usage, reports input but no output: a model that read its input wrote at least a
  /// token, so its output is unknown, not none. Claude Code on an Anthropic-compatible API such as DeepSeek's reports
  /// output only in the result that ends its turn (its messages carry the provider's `message_start` count, 0), so
  /// a turn still at work, or one stopped before its result, has no output yet.
  pub const fn output_unreported(self) -> bool {
    self.input > 0 && self.output == 0
  }
}

impl AddAssign for TokenUsage {
  fn add_assign(&mut self, other: Self) {
    self.input = self.input.saturating_add(other.input);
    self.cached_input = self.cached_input.saturating_add(other.cached_input);
    self.output = self.output.saturating_add(other.output);
  }
}

impl std::iter::Sum for TokenUsage {
  fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
    iter.fold(Self::default(), |mut total, usage| {
      total += usage;
      total
    })
  }
}
