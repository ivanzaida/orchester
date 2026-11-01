//! Settings › General showing its *Updates* (`wSMNG` in `Auto-updater · redesign` `acFkz`): what the status bar's
//! update item and the ready notification open, `/settings?tab=general&show=updates`, or with the release notes open
//! after an update, `&show=whats-new`. `&show=plan-limits` shows its *Plan limits* instead, as Usage › Plan limits'
//! menu opens it.

use crate::routes::RouteError;

/// The query parameter that names what General shows.
pub(super) const SHOW: &str = "show";
/// What General shows when Usage › Plan limits' menu opens it: its *Plan limits*, where the warning threshold is set.
pub(super) const PLAN_LIMITS: &str = "plan-limits";

/// What Settings › General shows of its *Updates*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UpdatesShow {
  /// The section, scrolled into view.
  Section,
  /// The section with its release notes open.
  WhatsNew,
}

impl UpdatesShow {
  pub const ALL: [Self; 2] = [Self::Section, Self::WhatsNew];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Section => "updates",
      Self::WhatsNew => "whats-new",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|show| show.name() == name)
  }

  /// What `name`, the value of [`SHOW`], names of *Updates*.
  pub(super) fn named(name: String) -> Result<Self, RouteError> {
    Self::from_name(&name).ok_or(RouteError::InvalidParam {
      param: SHOW.to_owned(),
      value: name,
    })
  }
}
