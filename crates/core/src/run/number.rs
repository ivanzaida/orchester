use std::{fmt, num::NonZeroU32};

use serde::{Deserialize, Serialize};

/// Display number of a run within its project (`#128`), from the project's run counter.
/// Runs are referenced by `RunId`; this is for people.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunNumber(NonZeroU32);

impl RunNumber {
  pub const fn new(number: NonZeroU32) -> Self {
    Self(number)
  }

  pub const fn get(self) -> u32 {
    self.0.get()
  }
}

impl fmt::Display for RunNumber {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "#{}", self.0)
  }
}
