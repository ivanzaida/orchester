//! The days a date picker allows.

use chrono::NaiveDate;

/// The days a date may be: not before `earliest`, not after `latest`. A Target date's calendar has the Start date as
/// its earliest, a Start date's the Target date as its latest; the module keeps its own check anyway.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Limits {
  pub earliest: Option<NaiveDate>,
  pub latest: Option<NaiveDate>,
}

impl Limits {
  pub const NONE: Self = Self {
    earliest: None,
    latest: None,
  };

  /// Whether `day` can be picked.
  pub fn allows(self, day: NaiveDate) -> bool {
    self.earliest.is_none_or(|earliest| day >= earliest) && self.latest.is_none_or(|latest| day <= latest)
  }
}
