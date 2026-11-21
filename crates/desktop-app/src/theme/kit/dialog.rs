//! A confirmation in a modal (`ui::dialog`, `Modal layer` `ZsL7y` in `04d · Bind an MCP server` `z0iFy`): lurq's
//! `ui::Dialog` over the `scrim`, under the title bar that lurq's window chrome keeps. The panel is 500 wide on
//! `surface_raised` in a `border_strong` stroke, radius 12, with the large shadow. Its header is 20 in from the
//! panel's top and sides: a 32 tile, radius 8, in `warning_muted` with a 16 glyph in `warning` (or another tone's), 16
//! before the question in 15/600 and what it is about 2 under it in 11.5 `text_muted`. Its body is 14 under the header
//! and 20 at the sides, its parts 14 apart, 24 over the footer; a dialog with no body has those 24 under its header.
//! The footer, under a `border` line, is 16 over and under and 20 at the sides, its buttons 8 apart; its note says what
//! Enter and Esc do in 11.5 `text_muted` beside their caps, 6 from each, the pairs 12 apart. A bare dialog
//! ([`BARE`]) is the caller's own parts in the panel, 20 inside it and 14 apart, radius 8.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, Theme},
  node::border::Border,
  ui::{Dialog, DialogPart, DialogState, Length, Recipe},
};

use crate::theme::{
  DIALOG_TILE, DIALOG_WIDTH, DialogSpacing, DialogText, ExtraColor, ExtraRadius, ExtraSpacing, IconSize,
};

/// The tile's tones over the default `warning`: `info`, `danger` and a neutral `hover` with a `text_secondary` glyph.
pub const INFO: &str = "info";
pub const DANGER: &str = "danger";
pub const NEUTRAL: &str = "neutral";
/// A dialog of a header and a footer alone: the header keeps the body's room under it.
pub const BODILESS: &str = "bodiless";
/// The caller's own parts in the panel, with no header or footer of the dialog's.
pub const BARE: &str = "bare";

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<Dialog>(tones(states(base())));
}

fn base() -> Recipe {
  let side = DialogSpacing::Side.role();
  Recipe::new()
    .base(DialogPart::Veil, |style| style.background(ExtraColor::Scrim.role()))
    .base(DialogPart::Panel, |style| {
      style
        .width(Length::Px(DIALOG_WIDTH))
        .background(PaletteColor::Card)
        .foreground(PaletteColor::Foreground)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Input))
        .radius(RadiusSize::Lg)
        .shadow(ShadowStyle::Lg)
    })
    .base(DialogPart::Header, |style| {
      style.gap(SpacingSize::Md).padding_top(side).padding_x(side)
    })
    .base(DialogPart::Mark, |style| {
      style
        .square(Length::Px(DIALOG_TILE))
        .radius(ExtraRadius::Tile.role())
        .background(PaletteColor::WarningMuted)
    })
    .base(DialogPart::Icon, |style| {
      style
        .square(Length::Px(IconSize::Lg.size()))
        .foreground(PaletteColor::Warning)
    })
    .base(DialogPart::Text, |style| style.gap(ExtraSpacing::Tight.role()))
    .base(DialogPart::Title, |style| style.typography(DialogText::Title.role()))
    .base(DialogPart::Description, |style| {
      style.typography(DialogText::Detail.role())
    })
    .base(DialogPart::Body, |style| {
      style
        .gap(DialogSpacing::Parts.role())
        .padding_top(DialogSpacing::Parts.role())
        .padding_x(side)
        .padding_bottom(SpacingSize::Lg)
    })
    .base(DialogPart::Footer, |style| {
      style
        .gap(SpacingSize::Sm)
        .padding_x(side)
        .padding_y(SpacingSize::Md)
        .top_line(Border::inside(BorderSize::Sm, PaletteColor::Border))
    })
    .base(DialogPart::Note, |style| {
      style.gap(ExtraSpacing::Snug.role()).typography(DialogText::Keys.role())
    })
    .base(DialogPart::NoteKey, |style| {
      style
        .gap(ExtraSpacing::Snug.role())
        .padding_start(ExtraSpacing::Snug.role())
    })
}

fn states(rules: Recipe) -> Recipe {
  rules
    .state(DialogState::Mark, DialogPart::Header, |style| {
      style.gap(SpacingSize::Md)
    })
    .state(BODILESS, DialogPart::Header, |style| {
      style.padding_bottom(SpacingSize::Lg)
    })
    .state(BARE, DialogPart::Panel, |style| {
      style
        .width(Length::Auto)
        .radius(ExtraRadius::Tile.role())
        .padding_x(DialogSpacing::Side.role())
        .padding_y(DialogSpacing::Side.role())
    })
    .state(BARE, DialogPart::Body, |style| {
      style.padding_top(0.0).padding_x(0.0).padding_bottom(0.0)
    })
}

fn tones(rules: Recipe) -> Recipe {
  [
    (INFO, PaletteColor::InfoMuted, PaletteColor::Info),
    (DANGER, PaletteColor::DestructiveMuted, PaletteColor::Destructive),
    (NEUTRAL, PaletteColor::Highlight, ExtraColor::TextSecondary.role()),
  ]
  .into_iter()
  .fold(rules, |rules, (tone, fill, glyph)| {
    rules
      .state(tone, DialogPart::Mark, |style| style.background(fill))
      .state(tone, DialogPart::Icon, |style| style.foreground(glyph))
  })
}
