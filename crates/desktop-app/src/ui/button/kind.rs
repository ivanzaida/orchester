//! The look of a button: its fill, stroke and content color, and how it answers the pointer.

use lurq::{
  app::theme::{BorderSize, PaletteColor},
  components::Button,
  node::{Style, border::Border},
};

use crate::theme::ExtraColor;

/// The look of a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
  /// The one action a screen or card is about: `accent`, its content `on_accent`.
  Primary,
  /// `surface_raised` inside a `border_strong` stroke, its content `text_primary`.
  Secondary,
  /// No fill: its content `text_secondary`, the `hover` fill under the pointer.
  Ghost,
  /// `danger_muted`, its content `danger`.
  Danger,
  /// Destructive text on the ghost button (Secrets inspector, Emd0h).
  DangerGhost,
  /// A field-like menu trigger with a trailing chevron (LWah7, Runs after).
  Select,
}

impl ButtonKind {
  pub const fn name(self) -> &'static str {
    match self {
      Self::Primary => "primary",
      Self::Secondary => "secondary",
      Self::Ghost => "ghost",
      Self::Danger => "danger",
      Self::DangerGhost => "danger-ghost",
      Self::Select => "select",
    }
  }

  /// The color of the icon and the label.
  pub fn content(self) -> PaletteColor {
    match self {
      Self::Primary => ExtraColor::OnAccent.role(),
      Self::Secondary | Self::Select => PaletteColor::Foreground,
      Self::Ghost => ExtraColor::TextSecondary.role(),
      Self::Danger | Self::DangerGhost => PaletteColor::Destructive,
    }
  }

  pub(super) fn dressed(self, button: Button) -> Button {
    match self {
      Self::Primary => button.background(PaletteColor::Primary),
      Self::Secondary => button
        .background(PaletteColor::Card)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Input)),
      Self::Ghost | Self::DangerGhost => button,
      Self::Select => button
        .background(ExtraColor::InputSurface.role())
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border)),
      Self::Danger => button.background(PaletteColor::DestructiveMuted),
    }
  }

  /// Under the pointer and while pressed. The design draws neither; they follow the shell's `hover` and
  /// `pressed` overlays and the accent's own hover.
  pub(super) fn interactive(self, button: Button) -> Button {
    let fill = |color: PaletteColor| Style::new().background(color);
    match self {
      Self::Primary => button
        .hovered_style(fill(PaletteColor::PrimaryHover))
        .active_style(fill(PaletteColor::PrimaryHover)),
      Self::Secondary | Self::Select => button
        .hovered_style(fill(PaletteColor::Border))
        .active_style(fill(PaletteColor::Input)),
      Self::Ghost | Self::DangerGhost => button
        .hovered_style(fill(ExtraColor::Hover.role()))
        .active_style(fill(ExtraColor::Pressed.role())),
      Self::Danger => button
        .hovered_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::Destructive)))
        .active_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::Destructive))),
    }
  }
}

/// How high a button is, with the room at its sides and the size of its label: a size of the theme's recipe of
/// lurq's `ui::Button` (`theme::kit::button`), apart from lurq's own sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSize {
  /// 28 high, 12 at each side.
  Regular,
  /// As high as a heading's line (`Hfjkt`).
  Heading,
  /// 22 high with a note-size label (`ZdkB0`).
  Helper,
  /// 24 high (`PEnj5`).
  Small,
  /// 26 high (`SYiTG`).
  Snug,
  /// 30 high (`psiyx`).
  Pinned,
  /// 36 high with a 13/500 label, 14 at each side, 16 for a primary one (`rDzhV`, `fZDtO`).
  Large,
}

impl ButtonSize {
  pub const ALL: [Self; 7] = [
    Self::Regular,
    Self::Heading,
    Self::Helper,
    Self::Small,
    Self::Snug,
    Self::Pinned,
    Self::Large,
  ];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Regular => "regular",
      Self::Heading => "heading",
      Self::Helper => "helper",
      Self::Small => "small",
      Self::Snug => "snug",
      Self::Pinned => "pinned",
      Self::Large => "large",
    }
  }
}

/// The states of a button the theme's recipe names, beside lurq's `disabled`, `loading` and `full_width`.
pub mod state {
  /// 10 at each side instead of 12 (`SHKod`).
  pub const TIGHT: &str = "tight";
  /// Large and not primary, with a primary's room at each side.
  pub const ROOMY: &str = "roomy";
  /// Drawn as it is, but it no longer takes a press.
  pub const SHOWN: &str = "shown";
  /// Drawn pressed while what it opened shows.
  pub const HELD: &str = "held";
  /// The menu it opened shows.
  pub const MENU_OPEN: &str = "menu_open";
  /// What it sets applies something now.
  pub const APPLIED: &str = "applied";
  /// What it holds is refused.
  pub const INVALID: &str = "invalid";
}
