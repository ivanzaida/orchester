//! Settings › General showing its *Updates* (`wSMNG` in `Auto-updater · redesign` `acFkz`): what the status bar's
//! update item and the ready notification open, `/settings?tab=general&show=updates`, or with the release notes open
//! after an update, `&show=whats-new`.

use super::query::Params;
use crate::routes::RouteError;

/// The query parameter that names what General shows.
pub(super) const SHOW: &str = "show";

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

  /// What `params` names, if anything.
  pub(super) fn take(params: &mut Params) -> Result<Option<Self>, RouteError> {
    let Some(name) = params.take(SHOW)? else {
      return Ok(None);
    };
    Self::from_name(&name).map(Some).ok_or(RouteError::InvalidParam {
      param: SHOW.to_owned(),
      value: name,
    })
  }
}
