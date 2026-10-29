//! The design values of the *Browser* window (`Browser · redesign` `lcNez` in the new `orchester.pen`, ORC-379) that
//! the UI kit's and the shell's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
  node::color::Color,
};

use super::{
  Mode,
  typography::{mono, style},
};

/// Colors of the Browser window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserColor {
  /// `surface_base` fully transparent: where a cut edge's fade of the tab strip ends (`Fade` in `01e` `cmofl`).
  FadeClear,
}

impl BrowserColor {
  pub const ALL: [Self; 1] = [Self::FadeClear];

  pub fn role(self) -> PaletteColor {
    PaletteColor::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::FadeClear => "browser_fade_clear",
    }
  }

  fn color(self, mode: Mode) -> Color {
    match (self, mode) {
      (Self::FadeClear, Mode::Dark) => Color::from_hex("#11111300"),
      (Self::FadeClear, Mode::Light) => Color::from_hex("#E4E4E800"),
    }
  }
}

/// Text of the Browser window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserText {
  /// 10.5/600 `text_secondary`: the shown project's initial in the switcher's tile (`Initial` `sdrrt` in `u4BK9L`).
  SwitcherInitial,
  /// Geist Mono 12 `text_primary`: the address of the tab shown (`URL` `dSm2I` in `fPcCX`).
  Address,
  /// 13 `text_primary`, 1.5: what a page's JavaScript dialog says (`Message` `fCL4z` in `t08Ur`).
  DialogMessage,
  /// 11/500 `text_inverse`: the agent's name on its mark where it acted (`Text` `u7jZ2` in `Agent cursor` `D7l36`).
  AgentTag,
}

impl BrowserText {
  pub const ALL: [Self; 4] = [
    Self::SwitcherInitial,
    Self::Address,
    Self::DialogMessage,
    Self::AgentTag,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::SwitcherInitial => "browser_switcher_initial",
      Self::Address => "browser_address",
      Self::DialogMessage => "browser_dialog_message",
      Self::AgentTag => "browser_agent_tag",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::SwitcherInitial => style(10.5, FontWeight::SemiBold, 1.35, 0.0, palette.text_secondary),
      Self::Address => mono(style(12.0, FontWeight::Normal, 1.35, 0.0, palette.text_primary)),
      Self::DialogMessage => style(13.0, FontWeight::Normal, 1.5, 0.0, palette.text_primary),
      Self::AgentTag => style(11.0, FontWeight::Medium, 1.25, 0.0, palette.text_inverse),
    }
  }
}

/// Gaps and paddings of the Browser window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserSpacing {
  /// 7: between the switcher's tile, the project's name and its chevron (`Project switcher` `u4BK9L`).
  Switcher,
  /// 14: at each side of the line under the toolbar that says what the agent did (`Activity` `WTzV9`).
  Activity,
  /// 20: inside a page's JavaScript dialog (`Dialog · confirm` `Mcxt7`).
  Dialog,
}

impl BrowserSpacing {
  pub const ALL: [Self; 3] = [Self::Switcher, Self::Activity, Self::Dialog];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Switcher => "browser_switcher",
      Self::Activity => "browser_activity",
      Self::Dialog => "browser_dialog",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Switcher => 7.0,
      Self::Activity => 14.0,
      Self::Dialog => 20.0,
    }
  }
}

/// Strokes of the Browser window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserBorder {
  /// 1.5: the ring of a tab's dot in the title bar's color, and the ring around the element an agent acted on
  /// (`Indicator · new` `XZLhY`, `Agent mark` `XczoY`).
  Ring,
}

impl BrowserBorder {
  pub const ALL: [Self; 1] = [Self::Ring];

  pub fn role(self) -> BorderSize {
    BorderSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Ring => "browser_ring",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Ring => 1.5,
    }
  }
}

pub(super) fn colors(mode: Mode) -> impl Iterator<Item = (Arc<str>, Color)> {
  BrowserColor::ALL
    .into_iter()
    .map(move |color| (Arc::from(color.name()), color.color(mode)))
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  BrowserText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in BrowserSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
  for border in BrowserBorder::ALL {
    theme.set_border_size_value(border.role(), border.value());
  }
}
