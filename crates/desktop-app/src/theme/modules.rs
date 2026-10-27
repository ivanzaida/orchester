//! The design values of the Modules pages (`Modules · redesign` `oEELJ` in the new `orchester.pen`, ORC-112) that the
//! UI kit's and the task page's roles do not carry, registered as lurq `Extra` roles.

use lurq::app::theme::{SpacingSize, Theme};

/// Gaps and paddings of the Modules pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulesSpacing {
  /// 18: the Summary's properties inside their left hairline, their rows' 6 making the task Summary's 24 (`Properties`
  /// `ihSv0`).
  Properties,
}

impl ModulesSpacing {
  pub const ALL: [Self; 1] = [Self::Properties];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Properties => "modules_properties",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Properties => 18.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in ModulesSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
