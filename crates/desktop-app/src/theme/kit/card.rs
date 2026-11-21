//! Card (`ui::card`; `Raise the limit` `xHs82`, `Approval card` `gwOaJ`): lurq's `ui::Card`, radius 10 inside a 1
//! stroke with no shadow, its content as the app pads it and its children with no room between them unless the app
//! says. Each surface is a variant: its fill and its stroke. The recipe is the app's whole: lurq's padding, gaps,
//! shadows and pressed edge are not the design's.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, Theme},
  node::border::Border,
  ui::{Card, CardPart, Length, Recipe},
};

/// The variants a card's surface names.
pub const PANEL: &str = "panel";
pub const RAISED: &str = "raised";
pub const OUTLINE: &str = "outline";
pub const DECISION: &str = "decision";
pub const ATTENTION: &str = "attention";
pub const PENDING: &str = "pending";
pub const STOPPED: &str = "stopped";

/// Each surface's fill and stroke.
const SURFACES: [(&str, Option<PaletteColor>, PaletteColor); 7] = [
  (PANEL, Some(PaletteColor::Canvas), PaletteColor::Border),
  (RAISED, Some(PaletteColor::Card), PaletteColor::Border),
  (OUTLINE, None, PaletteColor::Border),
  (DECISION, None, PaletteColor::Input),
  (ATTENTION, Some(PaletteColor::Card), PaletteColor::Warning),
  (PENDING, None, PaletteColor::Warning),
  (STOPPED, Some(PaletteColor::Card), PaletteColor::Destructive),
];

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<Card>(recipe());
}

fn recipe() -> Recipe {
  let recipe = Recipe::new()
    .base(CardPart::Root, |style| style.width(Length::Fill).radius(RadiusSize::Lg))
    .base(CardPart::Content, |style| style.padding_x(0.0).padding_y(0.0).gap(0.0))
    .base(CardPart::Body, |style| style.gap(0.0));
  SURFACES.into_iter().fold(recipe, |recipe, (surface, fill, stroke)| {
    recipe.variant(surface, CardPart::Root, |style| {
      let style = style.border(Border::inside(BorderSize::Sm, stroke));
      match fill {
        Some(fill) => style.background(fill),
        None => style,
      }
    })
  })
}
