use lurq::{
  app::theme::{PaletteColor, ThemePalette},
  node::color::Color,
};

use super::tokens::{
  ACCENT, BORDER, CANVAS, FOCUS_GAP, FOCUS_RING, HAIRLINE, HOVER, INVERSE_TEXT, MUTED, RAISED, SCRIM, SIDEBAR, SUBTLE,
  SURFACE, TEXT,
};

/// Pencil colors without a built-in lurq role, registered as `PaletteColor::Extra`.
/// Use these instead of spelling extra names at call sites.
#[derive(Clone, Copy)]
pub enum ExtraColor {
  /// Cards and panels on the canvas (Pencil `surface`, e.g. `iN90G`).
  Surface,
  /// Row and section dividers (Pencil `hairline`, e.g. `zemhy`).
  Hairline,
  /// Dims the window behind a dialog (`a8U756`).
  Scrim,
  /// The light ring of a focused control (Pencil `focus-ring`, `xURvJ`).
  FocusRing,
  /// The dark gap between a focused control and its ring (Pencil `focus-gap`, `xURvJ`).
  FocusGap,
}

impl ExtraColor {
  const ALL: [Self; 5] = [
    Self::Surface,
    Self::Hairline,
    Self::Scrim,
    Self::FocusRing,
    Self::FocusGap,
  ];

  fn name(self) -> &'static str {
    match self {
      Self::Surface => "surface",
      Self::Hairline => "hairline",
      Self::Scrim => "scrim",
      Self::FocusRing => "focus-ring",
      Self::FocusGap => "focus-gap",
    }
  }

  fn value(self) -> Color {
    Color::from_hex(match self {
      Self::Surface => SURFACE,
      Self::Hairline => HAIRLINE,
      Self::Scrim => SCRIM,
      Self::FocusRing => FOCUS_RING,
      Self::FocusGap => FOCUS_GAP,
    })
  }

  pub fn role(self) -> PaletteColor {
    PaletteColor::extra(self.name())
  }
}

pub fn palette() -> ThemePalette {
  let c = Color::from_hex;
  let mut palette = ThemePalette {
    accent: c(ACCENT),
    // Not specified in the design: hover brightens the primary action towards `text`.
    accent_hover: c(TEXT),
    accent_muted: c(HOVER),
    surface_base: c(CANVAS),
    surface_panel: c(SIDEBAR),
    surface_raised: c(RAISED),
    // Fields are filled with `sidebar` in the design (`ZGwlv`).
    surface_input: c(SIDEBAR),
    border: c(BORDER),
    // Not specified in the design; the strongest neutral below `muted`.
    border_strong: c(SUBTLE),
    // Focused fields use a `text` stroke (`o8Oug`).
    border_focus: c(TEXT),
    text_primary: c(TEXT),
    text_secondary: c(MUTED),
    text_muted: c(SUBTLE),
    text_inverse: c(INVERSE_TEXT),
    // The design has no status colors (status pairs an icon with a label), so the
    // success/warning/danger/info roles keep lurq defaults until it defines them.
    ..ThemePalette::default()
  };
  for extra in ExtraColor::ALL {
    palette.extra.insert(extra.name().into(), extra.value());
  }
  palette
}
