//! Chips (`ui::chip`). A tag (`Chip` `jI8af`) is lurq's `ui::Tag`: its name in 11/500 `text_secondary` on a
//! `surface_raised` pill inside a `border` stroke, 2 over and under it and 8 at its sides; a removable one ends in an
//! 11 `text_muted` cross, 4 after the name and 4 from its end (`Chip/removable` `qj3Ss`); one that says what something
//! is leads with an 11 `text_secondary` glyph, 4 before the name (`Chip · Decision` `HWxlV`); an identifier is in
//! Geist Mono (`XRCed`); a form's pill is 24 high (`TnP0h`). A badge (`Badge` `Fm7lc`) is lurq's `ui::Badge`: a 6 dot
//! or a 12 glyph and a word in 11/500 in its tone, 6 apart, on the tone's muted fill, 3 over and under them and 8 at
//! their sides. Both recipes are the app's whole: lurq's own looks of tags and badges are not the design's.

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, Theme, TypographyStyle},
  node::{CursorIcon, border::Border},
  ui::{Badge, BadgePart, Length, Recipe, Tag, TagPart, TagState},
};

use super::plain::plain;
use crate::theme::{ControlHeight, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize};

/// A badge's dot (`Vydkx`).
const DOT_SIZE: f32 = 6.0;

/// The state of a tag whose name is an identifier, in Geist Mono.
pub const MONO: &str = "mono";
/// The state of a form's pill, 24 high.
pub const FIELD: &str = "field";

/// The variants of a badge, one per tone: its word's and its dot's color over the fill.
pub const SUCCESS: &str = "success";
pub const INFO: &str = "info";
pub const WARNING: &str = "warning";
pub const DANGER: &str = "danger";
pub const NEUTRAL: &str = "neutral";

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<Tag>(tag());
  theme.set_recipe::<Badge>(badge());
}

fn tag() -> Recipe {
  let icon = Length::Px(IconSize::Xs.size());
  Recipe::new()
    .base(TagPart::Root, |style| {
      style
        .padding_x(SpacingSize::Sm)
        .padding_y(ExtraSpacing::Tight.role())
        .gap(SpacingSize::Xs)
        .radius(ExtraRadius::Full.role())
        .background(PaletteColor::Card)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .foreground(ExtraColor::TextSecondary.role())
    })
    .base(TagPart::Label, |style| {
      style.typography(plain(TypographyStyle::FieldLabel))
    })
    .base(TagPart::Icon, |style| style.square(icon))
    .base(TagPart::Remove, |style| {
      style
        .square(icon)
        .foreground(PaletteColor::MutedForeground)
        .cursor(CursorIcon::Pointer)
    })
    .base(TagPart::RemoveIcon, |style| style.square(icon))
    .state(TagState::Remove, TagPart::Root, |style| {
      style.padding_end(SpacingSize::Xs)
    })
    .state(MONO, TagPart::Label, |style| {
      style.typography(plain(ExtraTypography::MonoLabel.role()))
    })
    .state(FIELD, TagPart::Root, |style| style.height(ControlHeight::Sm.value()))
}

fn badge() -> Recipe {
  let tones = [
    (SUCCESS, PaletteColor::Success, PaletteColor::SuccessMuted),
    (INFO, PaletteColor::Info, PaletteColor::InfoMuted),
    (WARNING, PaletteColor::Warning, PaletteColor::WarningMuted),
    (DANGER, PaletteColor::Destructive, PaletteColor::DestructiveMuted),
    (NEUTRAL, ExtraColor::TextSecondary.role(), PaletteColor::Highlight),
  ];
  let recipe = Recipe::new()
    .base(BadgePart::Root, |style| {
      style
        .padding_x(SpacingSize::Sm)
        .padding_y(ExtraSpacing::Fine.role())
        .gap(ExtraSpacing::Snug.role())
        .radius(ExtraRadius::Full.role())
    })
    .base(BadgePart::Label, |style| {
      style.typography(plain(TypographyStyle::FieldLabel))
    })
    .base(BadgePart::Icon, |style| style.square(Length::Px(IconSize::Sm.size())))
    .base(BadgePart::Dot, |style| {
      style.square(DOT_SIZE).radius(ExtraRadius::Full.role())
    });
  tones.into_iter().fold(recipe, |recipe, (tone, strong, muted)| {
    let dot = strong.clone();
    recipe
      .variant(tone, BadgePart::Root, |style| {
        style.background(muted).foreground(strong)
      })
      .variant(tone, BadgePart::Dot, |style| style.background(dot))
  })
}
