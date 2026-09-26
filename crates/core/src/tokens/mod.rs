//! Token budgets (`docs/design/orchester-workspace.md`, "Token budgets"): every budget is a whole
//! number of tokens, never money. What counts toward a budget is input + output as the CLIs
//! report it ([`crate::run::TokenUsage::counted`]); cached input is shown but not counted.

mod input;

use std::{
  fmt,
  num::NonZeroU64,
  ops::{Add, AddAssign},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::input::{BudgetInputError, digits_only, parse_budget, regroup};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenBudgetError {
  #[error("a token budget must be greater than zero")]
  Zero,
}

/// A budget of tokens, greater than zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct TokenBudget(NonZeroU64);

impl TokenBudget {
  pub fn new(tokens: u64) -> Result<Self, TokenBudgetError> {
    NonZeroU64::new(tokens).map(Self).ok_or(TokenBudgetError::Zero)
  }

  /// For constants: `None` for zero.
  pub const fn from_const(tokens: u64) -> Option<Self> {
    match NonZeroU64::new(tokens) {
      Some(tokens) => Some(Self(tokens)),
      None => None,
    }
  }

  pub const fn get(self) -> u64 {
    self.0.get()
  }
}

impl TryFrom<u64> for TokenBudget {
  type Error = TokenBudgetError;

  fn try_from(tokens: u64) -> Result<Self, TokenBudgetError> {
    Self::new(tokens)
  }
}

impl From<TokenBudget> for u64 {
  fn from(budget: TokenBudget) -> Self {
    budget.get()
  }
}

impl fmt::Display for TokenBudget {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.get().fmt(f)
  }
}

/// Tokens that counted toward a budget. Usage only grows, so adding saturates instead of
/// wrapping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TokensUsed(u64);

impl TokensUsed {
  pub const ZERO: Self = Self(0);

  pub const fn new(tokens: u64) -> Self {
    Self(tokens)
  }

  pub const fn get(self) -> u64 {
    self.0
  }

  /// Whether this much usage has used up `budget`. Usage arrives after a turn, so it can end
  /// above the budget; it is never clamped.
  pub const fn reached(self, budget: TokenBudget) -> bool {
    self.0 >= budget.get()
  }

  /// Tokens left of `budget`, zero once it is used up.
  pub const fn left_of(self, budget: TokenBudget) -> u64 {
    budget.get().saturating_sub(self.0)
  }
}

impl Add for TokensUsed {
  type Output = Self;

  fn add(self, other: Self) -> Self {
    Self(self.0.saturating_add(other.0))
  }
}

impl AddAssign for TokensUsed {
  fn add_assign(&mut self, other: Self) {
    *self = *self + other;
  }
}

impl std::iter::Sum for TokensUsed {
  fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
    iter.fold(Self::ZERO, Add::add)
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

/// `value` with its digits grouped by thousands with `separator`, the locale's group separator:
/// `1,400,000` in English.
pub fn grouped(value: u64, separator: &str) -> String {
  group_digit_text(&value.to_string(), separator)
}

/// A string of ASCII digits grouped by thousands from the right.
fn group_digit_text(digits: &str, separator: &str) -> String {
  let count = digits.len();
  let mut grouped = String::with_capacity(count + count / 3 * separator.len());
  for (position, digit) in digits.chars().enumerate() {
    if position > 0 && (count - position).is_multiple_of(3) {
      grouped.push_str(separator);
    }
    grouped.push(digit);
  }
  grouped
}
