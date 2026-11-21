//! Notice (`ui::notice`, `ui::setting_notice`): lurq's `ui::Alert` in the looks of the kit's notices. Each tone is an
//! `AlertVariant`, its fill and its icon's color; the stroke is its tone's where the look has one (state `stroked`).
//! The recipe is the app's whole: lurq's 16 of padding, 14 titles and close control are not the design's.
//!
//! - In a row (`Notice` `R3Pyx`): a 16 icon, the title in 13/500 `text_primary` with its explanation 2 under it in
//!   12/1.45 `text_secondary`, the actions at the end 10 from the words and 8 apart; radius 10, 12 over and under and
//!   14 at its sides. Stacked (`SSbRV`), the explanation is 3 under the title and the parts and actions 10 under.
//! - Flat (`Mxt7k`, Settings' `TPqRf`): radius 6, 10 over and under and 12 at its sides, a 14 icon, its words, parts,
//!   actions and note 6 apart.
//! - Raised (`b3htnP`): on `surface_raised`, radius 6, a 15 icon, the explanation 4 under the title and the actions 10
//!   under the words at their end.
//! - In an inspector (compact, `adK4e`): radius 8, 10 over and under and 12 at its sides, a 13 icon beside a 13/500
//!   title, and under them, 6 apart, the explanation in 11.5 `text_secondary`, its parts and its actions.
//! - Handling (`mERxM`): radius 8, 10 over and under and 12 at its sides, a 15 `text_secondary` icon beside its words.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, Theme, TypographyStyle},
  node::border::Border,
  ui::{Alert, AlertPart, AlertState, AlertVariant, Length, Recipe, Selector},
};

use crate::theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize};

/// The state of a notice in its tone's stroke.
pub const STROKED: &str = "stroked";
/// The state of a flat notice.
pub const FLAT: &str = "flat";
/// The state of a raised notice.
pub const RAISED: &str = "raised";
/// The state of a handling notice.
pub const HANDLING: &str = "handling";

/// Each tone's fill, stroke and icon color.
const TONES: [(AlertVariant, PaletteColor, PaletteColor, PaletteColor); 5] = [
  (
    AlertVariant::Neutral,
    PaletteColor::Card,
    PaletteColor::Border,
    PaletteColor::MutedForeground,
  ),
  (
    AlertVariant::Info,
    PaletteColor::InfoMuted,
    PaletteColor::Info,
    PaletteColor::Info,
  ),
  (
    AlertVariant::Warning,
    PaletteColor::WarningMuted,
    PaletteColor::Warning,
    PaletteColor::Warning,
  ),
  (
    AlertVariant::Destructive,
    PaletteColor::DestructiveMuted,
    PaletteColor::Destructive,
    PaletteColor::Destructive,
  ),
  (
    AlertVariant::Success,
    PaletteColor::SuccessMuted,
    PaletteColor::Success,
    PaletteColor::Success,
  ),
];

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<Alert>(looks(tones(row())));
}

fn icon(size: IconSize) -> Length {
  Length::Px(size.size())
}

/// What the parts and the actions under the words stand from what is over them, where the words are `gap` apart.
fn under(gap: ExtraSpacing) -> f32 {
  ExtraSpacing::Loose.value() - gap.value()
}

/// A notice in a row, and stacked.
fn row() -> Recipe {
  Recipe::new()
    .base(AlertPart::Root, |style| {
      style
        .width(Length::Fill)
        .padding_x(ExtraSpacing::Inset.role())
        .padding_y(SpacingSize::Md)
        .gap(ExtraSpacing::Loose.role())
        .radius(RadiusSize::Lg)
    })
    .base(AlertPart::Icon, |style| style.square(icon(IconSize::Lg)))
    .base(AlertPart::Text, |style| style.gap(ExtraSpacing::Tight.role()))
    .base(AlertPart::Title, |style| {
      style.typography(ExtraTypography::Strong.role())
    })
    .base(AlertPart::Description, |style| {
      style.typography(ExtraTypography::Prose.role())
    })
    .base(AlertPart::Body, |style| style.gap(ExtraSpacing::Tight.role()))
    .base(AlertPart::Action, |style| style.gap(SpacingSize::Sm))
    .base(AlertPart::Note, |style| style.typography(ExtraTypography::Note.role()))
    .base(AlertPart::Head, |style| style.gap(ExtraSpacing::Snug.role()))
    .state(AlertState::Stacked, AlertPart::Text, |style| {
      style.gap(ExtraSpacing::Fine.role())
    })
    .state(AlertState::Stacked, AlertPart::Body, |style| {
      style
        .padding_top(under(ExtraSpacing::Fine))
        .gap(ExtraSpacing::Loose.role())
    })
    .state(AlertState::Stacked, AlertPart::Action, |style| {
      style.padding_top(under(ExtraSpacing::Fine))
    })
}

fn tones(recipe: Recipe) -> Recipe {
  TONES.into_iter().fold(recipe, |recipe, (tone, fill, stroke, mark)| {
    recipe
      .variant(tone, AlertPart::Root, |style| style.background(fill))
      .variant(tone, AlertPart::Icon, |style| style.foreground(mark))
      .when(Selector::new().variant(tone).state(STROKED), AlertPart::Root, |style| {
        style.border(Border::inside(BorderSize::Sm, stroke))
      })
  })
}

/// Words, parts and actions `gap` apart, none further from what is over them.
fn close_words(recipe: Recipe, look: impl Into<lurq::ui::Name> + Clone, gap: ExtraSpacing) -> Recipe {
  recipe
    .state(look.clone(), AlertPart::Text, |style| style.gap(gap.role()))
    .state(look.clone(), AlertPart::Body, |style| {
      style.padding_top(0.0).gap(gap.role())
    })
    .state(look, AlertPart::Action, |style| style.padding_top(0.0))
}

/// The flat, raised, inspector and handling looks, over the stacked one's.
fn looks(recipe: Recipe) -> Recipe {
  let quiet = |style: lurq::ui::PartStyle| style.padding_x(SpacingSize::Md).padding_y(ExtraSpacing::Loose.role());
  let recipe = recipe
    .state(FLAT, AlertPart::Root, |style| quiet(style).radius(RadiusSize::Md))
    .state(FLAT, AlertPart::Icon, |style| style.square(icon(IconSize::Md)))
    .state(RAISED, AlertPart::Root, |style| {
      style.background(PaletteColor::Card).radius(RadiusSize::Md)
    })
    .state(RAISED, AlertPart::Icon, |style| style.square(icon(IconSize::Nav)))
    .state(AlertState::Compact, AlertPart::Root, |style| {
      quiet(style)
        .gap(ExtraSpacing::Snug.role())
        .radius(ExtraRadius::Tile.role())
    })
    .state(AlertState::Compact, AlertPart::Icon, |style| {
      style.square(icon(IconSize::Base))
    })
    .state(AlertState::Compact, AlertPart::Title, |style| {
      style.typography(TypographyStyle::Label)
    })
    .state(AlertState::Compact, AlertPart::Description, |style| {
      style
        .typography(ExtraTypography::Note.role())
        .foreground(ExtraColor::TextSecondary.role())
    })
    .state(HANDLING, AlertPart::Root, |style| {
      quiet(style).radius(ExtraRadius::Tile.role())
    })
    .state(HANDLING, AlertPart::Icon, |style| {
      style
        .square(icon(IconSize::Nav))
        .foreground(ExtraColor::TextSecondary.role())
    });
  let recipe = close_words(recipe, FLAT, ExtraSpacing::Snug);
  let recipe = close_words(recipe, AlertState::Compact, ExtraSpacing::Snug);
  let recipe = close_words(recipe, HANDLING, ExtraSpacing::Snug);
  recipe
    .state(RAISED, AlertPart::Text, |style| style.gap(SpacingSize::Xs))
    .state(RAISED, AlertPart::Body, |style| {
      style.padding_top(0.0).gap(SpacingSize::Xs)
    })
    // 10 under the words, which are 4 apart: 6 more.
    .state(RAISED, AlertPart::Action, |style| {
      style.padding_top(ExtraSpacing::Snug.role())
    })
}
