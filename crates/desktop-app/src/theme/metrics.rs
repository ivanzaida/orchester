use lurq::app::theme::{BorderSize, RadiusSize, SpacingSize, Theme};

/// Design gaps and paddings without a built-in lurq role, registered as `SpacingSize::Extra`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraSpacing {
  /// 1: a project's name to the line under it (`Text` in `Project switcher`).
  Hair,
  /// 2: navigation items, tabs.
  Tight,
  /// 6: the title bar's start, a tab's icon to its label, a status item's icon to its label.
  Snug,
  /// 9: a navigation icon to its label.
  Inline,
  /// 10: the sidebar's bottom edge, an inactive tab's sides.
  Loose,
  /// 14: above a sidebar section's label (`Section label`).
  Section,
}

impl ExtraSpacing {
  pub const ALL: [Self; 6] = [
    Self::Hair,
    Self::Tight,
    Self::Snug,
    Self::Inline,
    Self::Loose,
    Self::Section,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Hair => "hair",
      Self::Tight => "tight",
      Self::Snug => "snug",
      Self::Inline => "inline",
      Self::Loose => "loose",
      Self::Section => "section_label",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Hair => 1.0,
      Self::Tight => 2.0,
      Self::Snug => 6.0,
      Self::Inline => 9.0,
      Self::Loose => 10.0,
      Self::Section => 14.0,
    }
  }
}

/// Design radii without a built-in lurq role, registered as `RadiusSize::Extra`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraRadius {
  /// 8: the project switcher (`Project switcher`).
  Tile,
  /// 8: the window frame (`q9mf0k`), the radius Windows 11 rounds windows to.
  Window,
  /// `radius_full` 999: pills and round avatars.
  Full,
}

impl ExtraRadius {
  pub const ALL: [Self; 3] = [Self::Tile, Self::Window, Self::Full];

  pub fn role(self) -> RadiusSize {
    RadiusSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Tile => "tile",
      Self::Window => "window",
      Self::Full => "full",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Tile | Self::Window => 8.0,
      Self::Full => 999.0,
    }
  }
}

/// `spacing_*`, `radius_*` and `border_size_*` of the design.
pub fn install(theme: &Theme) {
  for (size, value) in [
    (SpacingSize::Xs, 4.0),
    (SpacingSize::Sm, 8.0),
    (SpacingSize::Md, 12.0),
    (SpacingSize::Lg, 16.0),
    (SpacingSize::Xl, 24.0),
    (SpacingSize::Section, 32.0),
  ] {
    theme.set_spacing_value(size, value);
  }
  for extra in ExtraSpacing::ALL {
    theme.set_spacing_value(extra.role(), extra.value());
  }
  for (size, value) in [(RadiusSize::Sm, 4.0), (RadiusSize::Md, 6.0), (RadiusSize::Lg, 10.0)] {
    theme.set_radius_value(size, value);
  }
  for extra in ExtraRadius::ALL {
    theme.set_radius_value(extra.role(), extra.value());
  }
  for (size, value) in [(BorderSize::Sm, 1.0), (BorderSize::Md, 2.0), (BorderSize::Lg, 3.0)] {
    theme.set_border_size_value(size, value);
  }
}
