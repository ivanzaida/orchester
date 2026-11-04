//! The looks of a button ([`ButtonKind`]): its fill, stroke and content color, and its fills under the pointer and
//! while pressed.

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
  pub(super) const fn name(self) -> &'static str {
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
  pub(super) fn content(self) -> PaletteColor {
    match self {
      Self::Primary => ExtraColor::OnAccent.role(),
      Self::Secondary | Self::Select => PaletteColor::TextPrimary,
      Self::Ghost => PaletteColor::TextSecondary,
      Self::Danger | Self::DangerGhost => PaletteColor::Danger,
    }
  }

  pub(super) fn dressed(self, button: Button) -> Button {
    match self {
      Self::Primary => button.background(PaletteColor::Accent),
      Self::Secondary => button
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong)),
      Self::Ghost | Self::DangerGhost => button,
      Self::Select => button
        .background(PaletteColor::SurfaceInput)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border)),
      Self::Danger => button.background(PaletteColor::DangerMuted),
    }
  }

  /// Under the pointer and while pressed. The design draws neither; they follow the shell's `hover` and
  /// `pressed` overlays and the accent's own hover.
  pub(super) fn interactive(self, button: Button) -> Button {
    let fill = |color: PaletteColor| Style::new().background(color);
    match self {
      Self::Primary => button
        .hovered_style(fill(PaletteColor::AccentHover))
        .active_style(fill(PaletteColor::AccentHover)),
      Self::Secondary | Self::Select => button
        .hovered_style(fill(PaletteColor::Border))
        .active_style(fill(PaletteColor::BorderStrong)),
      Self::Ghost | Self::DangerGhost => button
        .hovered_style(fill(ExtraColor::Hover.role()))
        .active_style(fill(ExtraColor::Pressed.role())),
      Self::Danger => button
        .hovered_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::Danger)))
        .active_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::Danger))),
    }
  }
}
