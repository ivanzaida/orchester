//! Budget fields take absolute whole numbers only (`docs/design/orchester-workspace.md`, "Token
//! budgets", input rule): never `1.4M` or `250k`. The locale's group separator, spaces and
//! underscores are ignored; a decimal separator, a unit letter or anything else makes the value
//! invalid. There is no minimum beyond more than 0, and the project's maximum run budget bounds
//! every run budget.

use thiserror::Error;

use super::{TokenBudget, group_digit_text};

/// Why a budget field's text is not a budget. Each variant has its own message in the UI.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum BudgetInputError {
  #[error("no budget was entered")]
  Empty,
  #[error("a budget is written with digits only")]
  NotDigits,
  #[error("a budget must be above 0 tokens")]
  Zero,
  #[error("a run budget can be at most {max} tokens")]
  AboveMaximum { max: TokenBudget },
}

/// Characters a person may type between digits for readability: spaces (also the no-break
/// spaces some locales group with) and underscores, besides the locale's group separator.
const IGNORED: [char; 4] = [' ', '\u{00A0}', '\u{202F}', '_'];

/// The digits of `text` without group separators, spaces and underscores, or `None` when it
/// holds anything else (a decimal separator, a unit letter, a sign).
pub fn digits_only(text: &str, group_separator: &str) -> Option<String> {
  let without_groups = if group_separator.is_empty() {
    text.to_owned()
  } else {
    text.replace(group_separator, "")
  };
  let digits: String = without_groups.chars().filter(|c| !IGNORED.contains(c)).collect();
  digits.bytes().all(|byte| byte.is_ascii_digit()).then_some(digits)
}

/// Reads a budget field: a whole number of tokens above 0 and at most `max`.
pub fn parse_budget(text: &str, group_separator: &str, max: TokenBudget) -> Result<TokenBudget, BudgetInputError> {
  if text.trim().is_empty() {
    return Err(BudgetInputError::Empty);
  }
  let digits = digits_only(text, group_separator).ok_or(BudgetInputError::NotDigits)?;
  if digits.is_empty() {
    return Err(BudgetInputError::NotDigits);
  }
  // More digits than a u64 holds is certainly above any maximum.
  let tokens: u64 = digits.parse().map_err(|_| BudgetInputError::AboveMaximum { max })?;
  let budget = TokenBudget::new(tokens).map_err(|_| BudgetInputError::Zero)?;
  if budget > max {
    return Err(BudgetInputError::AboveMaximum { max });
  }
  Ok(budget)
}

/// `text` with its digits grouped for the locale while it is typed (en: `1400000` →
/// `1,400,000`), or `None` when it is not a whole number and stays as typed so its error can
/// name it. Leading zeros are kept: the field shows what was typed, only grouped.
pub fn regroup(text: &str, group_separator: &str) -> Option<String> {
  let digits = digits_only(text, group_separator)?;
  if digits.is_empty() {
    return None;
  }
  Some(group_digit_text(&digits, group_separator))
}
