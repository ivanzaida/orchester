//! Card: lurq's `ui::Card` in the kit's recipe (`theme::kit::card`), a block of related content, radius 10 inside a 1
//! stroke, 14 over and under its content and 16 at its sides (`Raise the limit` `xHs82`, `Approval card` `gwOaJ`). Its
//! surface says what it holds: a result on `surface_panel` (`Result card` `T0uKNU`), a figure or a request on
//! `surface_raised` (`Metric card` `Qhj0H`, `Needs-you card` `rkhZ2`), a list on the panel itself (`By step` `QuX1T`),
//! a decision the person makes in a `border_strong` stroke (`xHs82`), and what waits for them on `surface_raised` in a
//! `warning` stroke (`gwOaJ`, `Plan card` `Q5J0r`), or on the page itself in a `warning` stroke where the page is the
//! step (Local integration's `hULqu`). Callers add the content, its spacing (`gap`), and other padding where the design
//! gives it; a card of a height of its own gives it to its children.

use lurq::{app::theme::SpacingSize, node::padding::Padding, ui::Card};

use crate::theme::{ExtraSpacing, kit::card as recipe};

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
  /// No fill, in a `warning` stroke: the step of a page that waits for the person.
  Pending,
  /// `surface_raised` in a `danger` stroke: something stopped (`Run state · Stopped · archived` `pLdtd`).
  Stopped,
}

impl CardSurface {
  /// The variant of the card's recipe (`theme::kit::card`) that draws this surface.
  const fn variant(self) -> &'static str {
    match self {
      Self::Panel => recipe::PANEL,
      Self::Raised => recipe::RAISED,
      Self::Outline => recipe::OUTLINE,
      Self::Decision => recipe::DECISION,
      Self::Attention => recipe::ATTENTION,
      Self::Pending => recipe::PENDING,
      Self::Stopped => recipe::STOPPED,
    }
  }
}

/// A card on `surface` with the element id `id`, as wide as its parent; add its content as children.
pub fn card(id: &str, surface: CardSurface) -> Card {
  card_frame(id, surface).padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
}

/// A card whose header and body own their padding (`LruUv`, `VsylR`).
pub fn card_frame(id: &str, surface: CardSurface) -> Card {
  Card::new().id(id.to_owned()).variant(surface.variant())
}
