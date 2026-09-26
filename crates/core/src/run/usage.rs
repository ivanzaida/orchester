//! Token accounting (`docs/design/orchester-workspace.md`, "Token budgets"): counts come from
//! the usage each CLI reports, never from estimates. Counted = input + output; cached input is
//! shown but not counted. Codex reports cached tokens as part of input (OpenAI-style usage), so
//! counted input is input − cached.

use std::ops::AddAssign;

use serde::{Deserialize, Serialize};

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
  /// Input that counts toward a budget: input without the cached part.
  pub const fn counted_input(self) -> u64 {
    self.input.saturating_sub(self.cached_input)
  }

  /// Tokens that count toward a budget.
  pub const fn counted(self) -> u64 {
    self.counted_input().saturating_add(self.output)
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

/// A token count in the compact form of the display rule (specimen `T1n5BA`): the exact number
/// below 1,000, then `k`, `M` and `B`, with one decimal while the scaled value is below 100 and a
/// trailing `.0` dropped. Rounds half up; a value that rounds to 1,000 of a unit moves to the next
/// unit (`999,960` → `1M`). Unit letters are not translated.
pub fn compact_tokens(count: u64) -> String {
  const UNITS: [(u64, &str); 3] = [(1_000, "k"), (1_000_000, "M"), (1_000_000_000, "B")];
  if count < 1_000 {
    return count.to_string();
  }
  let mut index = UNITS
    .iter()
    .rposition(|(size, _)| count >= *size)
    .expect("count is at least 1,000");
  loop {
    let (size, letter) = UNITS[index];
    // Tenths of the unit, rounded half up, in integer arithmetic.
    let tenths = (u128::from(count) * 10 + u128::from(size) / 2) / u128::from(size);
    let (whole, fraction) = (tenths / 10, tenths % 10);
    if whole >= 1_000 && index + 1 < UNITS.len() {
      index += 1;
      continue;
    }
    if whole >= 100 {
      let rounded = (u128::from(count) + u128::from(size) / 2) / u128::from(size);
      if rounded >= 1_000 && index + 1 < UNITS.len() {
        index += 1;
        continue;
      }
      return format!("{rounded}{letter}");
    }
    return if fraction == 0 {
      format!("{whole}{letter}")
    } else {
      format!("{whole}.{fraction}{letter}")
    };
  }
}
