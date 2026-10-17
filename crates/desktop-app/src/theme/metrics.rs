use lurq::app::theme::{BorderSize, RadiusSize, SpacingSize, Theme};

/// Design gaps and paddings without a built-in lurq role, registered as `SpacingSize::Extra`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraSpacing {
  /// 1: a project's name to the line under it (`Text` in `Project switcher`).
  Hair,
  /// 2: navigation items, tabs; a segmented control's padding and segments (`C9VsTA`), a menu's options
  /// (`ClKqz`), a chip's top and bottom (`jI8af`), a menu option's label to its detail (`j6ZUhL`).
  Tight,
  /// 3: a badge's top and bottom (`Fm7lc`), a form row's label to its description (`XaNIz`).
  Fine,
  /// 5: a status, priority or stage icon to its word (`VWRbG`, `OfuTC`, `hDyRI`), a locked tab's lock to
  /// its label (`W4QYDM`).
  Pair,
  /// 6: the title bar's start, a tab's icon to its label, a status item's icon to its label; a button's
  /// icon to its label (`yJ2jo`), a field's label to its box and its hint (`JFXZg`), a menu to its
  /// control (`FpAul`), a menu option's top and bottom (`j6ZUhL`), a reason line's lock to its words.
  Snug,
  /// 7: a text area's top and bottom (`IBFli` in `s72p3L`).
  TextArea,
  /// 9: a navigation icon to its label.
  Inline,
  /// 10: the sidebar's bottom edge, an inactive tab's sides; a field's and a text segment's sides
  /// (`QWfRV`, `c4p0b`), a notice's icon to its text (`R3Pyx`).
  Loose,
  /// 14: above a sidebar section's label (`Section label`).
  Section,
  /// 14: a notice's sides (`R3Pyx`), a card's sides (`T0uKNU`).
  Inset,
  /// 20: between page tabs (`Workflow tabs` `bxhee`, `Page tabs` `BzK1B`).
  Tabs,
  /// 28: inside an empty state's card (`Empty project` `LeIOm`).
  Roomy,
  /// 56: above and below a loading line (`Reading` `O8pgq`).
  Loading,
  /// 2.5: between the bars of the Orchester mark (`Logo` `Jbirl`).
  BrandBars,
  /// 2: the app menu below the button that opens it.
  MenuOffset,
  /// 18: a page's heading to its tabs: settings (eDPtp, including trimmed text line boxes), All projects'
  /// Usage tab (`anSHC`).
  SettingsHead,
  /// 18: the index empty line's top (lNMHR).
  IndexEmpty,
  /// 9: a filter chip's start, before its facet (`Filter chip` `zjUM8`).
  ChipStart,
}

impl ExtraSpacing {
  pub const ALL: [Self; 18] = [
    Self::Hair,
    Self::Tight,
    Self::Fine,
    Self::Pair,
    Self::Snug,
    Self::TextArea,
    Self::Inline,
    Self::Loose,
    Self::Section,
    Self::Inset,
    Self::Tabs,
    Self::Roomy,
    Self::Loading,
    Self::BrandBars,
    Self::MenuOffset,
    Self::SettingsHead,
    Self::IndexEmpty,
    Self::ChipStart,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Hair => "hair",
      Self::Tight => "tight",
      Self::Fine => "fine",
      Self::Pair => "pair",
      Self::Snug => "snug",
      Self::TextArea => "text_area",
      Self::Inline => "inline",
      Self::Loose => "loose",
      Self::Section => "section_label",
      Self::Inset => "inset",
      Self::Tabs => "tabs",
      Self::Roomy => "roomy",
      Self::Loading => "loading",
      Self::BrandBars => "brand_bars",
      Self::MenuOffset => "menu_offset",
      Self::SettingsHead => "settings_head",
      Self::IndexEmpty => "index_empty",
      Self::ChipStart => "chip_start",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Hair => 1.0,
      Self::Tight => 2.0,
      Self::Fine => 3.0,
      Self::Pair => 5.0,
      Self::Snug => 6.0,
      Self::TextArea => 7.0,
      Self::Inline => 9.0,
      Self::Loose => 10.0,
      Self::Section | Self::Inset => 14.0,
      Self::Tabs => 20.0,
      Self::Roomy => 28.0,
      Self::Loading => 56.0,
      Self::BrandBars => 2.5,
      Self::MenuOffset => 2.0,
      Self::SettingsHead | Self::IndexEmpty => 18.0,
      Self::ChipStart => 9.0,
    }
  }
}

/// Design radii without a built-in lurq role, registered as `RadiusSize::Extra`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraRadius {
  /// 8: tiles and menus: the project switcher (`Project switcher`), an empty state's glyph (`q4tTW`), a
  /// select's or a menu's list (`ClKqz`).
  Tile,
  /// The window frame's corners: 8 on Windows (`q9mf0k`, the radius Windows 11 rounds windows to), 10 on
  /// macOS, where lurq clips the window at 10.
  Window,
  /// `radius_full` 999: pills and round avatars.
  Full,
  /// 5.4: the Orchester mark's tile.
  BrandTile,
  /// 1.75: the ends of the Orchester mark's bars, half their width.
  BrandBar,
  /// 2: a meter's track and fill (`Meter` `wtAQI`).
  Meter,
  /// 2: a color swatch in a legend or a filter's option (`Swatch` `p4TazJ`, `u1ZxRL`).
  Swatch,
  /// 3: a link's corners and its focus ring's (`Mjhop`), as the Markdown links'.
  Link,
}

impl ExtraRadius {
  pub const ALL: [Self; 8] = [
    Self::Tile,
    Self::Window,
    Self::Full,
    Self::BrandTile,
    Self::BrandBar,
    Self::Meter,
    Self::Swatch,
    Self::Link,
  ];

  pub fn role(self) -> RadiusSize {
    RadiusSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Tile => "tile",
      Self::Window => "window",
      Self::Full => "full",
      Self::BrandTile => "brand_tile",
      Self::BrandBar => "brand_bar",
      Self::Meter => "meter",
      Self::Swatch => "swatch",
      Self::Link => "link",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Tile => 8.0,
      Self::Window => WINDOW_RADIUS,
      Self::Full => 999.0,
      Self::BrandTile => 5.4,
      Self::BrandBar => 1.75,
      Self::Meter | Self::Swatch => 2.0,
      Self::Link => super::markdown::LINK_RADIUS,
    }
  }
}

#[cfg(not(target_os = "macos"))]
const WINDOW_RADIUS: f32 = 8.0;
#[cfg(target_os = "macos")]
const WINDOW_RADIUS: f32 = 10.0;

/// The design's control heights (`control_height_sm`, `control_height_md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlHeight {
  /// 20: the code viewer tree's *Collapse all* (`mJGHi` in `o1Uea4`).
  Xs,
  /// 22: the helper's Open Secrets action (ZdkB0).
  Helper,
  /// 24: a removable row's button (`ivlGM` in `o6h0L5`).
  Sm,
  /// 28: buttons, fields, selects, menu items (`yJ2jo`, `JFXZg`, `mAHGP`).
  Md,
  /// 26: an inspector's close (Secrets m1So5).
  Inspector,
  /// 36: a large button, as the welcome's actions (`rDzhV`, `fZDtO`).
  Lg,
  /// 26: a picker and a command trigger in a page's header (`ZQd2D`, `qXoBR`).
  Picker,
}

impl ControlHeight {
  pub const fn value(self) -> f32 {
    match self {
      Self::Xs => 20.0,
      Self::Helper => 22.0,
      Self::Sm => 24.0,
      Self::Md => 28.0,
      Self::Inspector => 26.0,
      Self::Lg => 36.0,
      Self::Picker => 26.0,
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
