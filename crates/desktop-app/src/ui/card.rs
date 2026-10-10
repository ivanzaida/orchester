//! Card: a block of related content, radius 10 inside a 1 stroke, 14 over and under its content and 16
//! at its sides (`Raise the limit` `xHs82`, `Approval card` `gwOaJ`). Its surface says what it holds:
//! a result on `surface_panel` (`Result card` `T0uKNU`), a figure or a request on `surface_raised`
//! (`Metric card` `Qhj0H`, `Needs-you card` `rkhZ2`), a list on the panel itself (`By step` `QuX1T`), a
//! decision the person makes in a `border_strong` stroke (`xHs82`), and what waits for them on
//! `surface_raised` in a `warning` stroke (`gwOaJ`, `Plan card` `Q5J0r`). Callers add the content, its
//! spacing, and other padding where the design gives it.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::Column,
  node::{border::Border, dimension::Dimension, padding::Padding},
};

use crate::theme::ExtraSpacing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardSurface {
  /// `surface_panel` in a `border` stroke.
  Panel,
  /// `surface_raised` in a `border` stroke.
  Raised,
  /// No fill, in a `border` stroke.
  Outline,
  /// No fill, in a `border_strong` stroke: a decision the person makes.
  Decision,
  /// `surface_raised` in a `warning` stroke: something waits for the person.
  Attention,
}

impl CardSurface {
  const fn colors(self) -> (Option<PaletteColor>, PaletteColor) {
    match self {
      Self::Panel => (Some(PaletteColor::SurfacePanel), PaletteColor::Border),
      Self::Raised => (Some(PaletteColor::SurfaceRaised), PaletteColor::Border),
      Self::Outline => (None, PaletteColor::Border),
      Self::Decision => (None, PaletteColor::BorderStrong),
      Self::Attention => (Some(PaletteColor::SurfaceRaised), PaletteColor::Warning),
    }
  }
}

/// A card on `surface` with the element id `id`, as wide as its parent; add its content as children.
pub fn card(id: &str, surface: CardSurface) -> Column {
  card_frame(id, surface).padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
}

/// A card whose header and body own their padding (`LruUv`, `VsylR`).
pub fn card_frame(id: &str, surface: CardSurface) -> Column {
  let (fill, stroke) = surface.colors();
  let card = Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Lg)
    .clip();
  match fill {
    Some(fill) => card.background(fill),
    None => card,
  }
}
