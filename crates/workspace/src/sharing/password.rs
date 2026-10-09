//! What a new password must be before it is sent (`docs/architecture/server.md`, "Accounts and
//! tokens": 12 to 256 characters, no composition rules); the server checks it again.

use std::ops::RangeInclusive;

/// How many characters a password has.
pub const PASSWORD_CHARS: RangeInclusive<usize> = 12..=256;

/// Why a new password and its repetition cannot be saved yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordProblem {
  /// Nothing typed yet.
  Empty,
  /// Shorter than 12 characters; it has `chars`.
  TooShort {
    chars: usize,
  },
  TooLong,
  /// The repetition is typed and differs.
  Different,
  /// The repetition is not typed yet.
  NotRepeated,
}

/// The problem of `new` repeated as `repeat`; `None` when it can be saved.
pub fn password_problem(new: &str, repeat: &str) -> Option<PasswordProblem> {
  let chars = new.chars().count();
  if chars == 0 {
    return Some(PasswordProblem::Empty);
  }
  if chars < *PASSWORD_CHARS.start() {
    return Some(PasswordProblem::TooShort { chars });
  }
  if chars > *PASSWORD_CHARS.end() {
    return Some(PasswordProblem::TooLong);
  }
  if repeat.is_empty() {
    return Some(PasswordProblem::NotRepeated);
  }
  (new != repeat).then_some(PasswordProblem::Different)
}
