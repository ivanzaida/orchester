//! Segmented control (`Segmented/text` `S5Fsve`, `Segmented` `C9VsTA`): lurq's `ui::SegmentedControl` in the kit's
//! recipe, which replaces lurq's: 26 high, `surface_input` inside a `border` stroke, radius 6, 2 inside and between
//! segments. A segment is radius 4, 10 at its sides, its label 12 `text_muted`; chosen, filled with `pressed` and its
//! label 12/500 `text_primary`; the others take the `hover` fill under the pointer. A count is 12 `text_muted`, 6
//! after the label; a glyph before the label 13 in the label's color. An icon segment is 28 wide, its 13 glyph
//! `text_primary` when chosen and `text_muted` otherwise, the chosen one filled with `hover`. A choice that warns has
//! an 11 `triangle-alert` in `danger` 5 before its label; chosen, on `danger_muted` with its label in `danger`.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, Theme, TypographyStyle},
  node::{CursorIcon, border::Border},
  ui::{Length, Recipe, SegmentedControl, SegmentedControlPart, SegmentedControlState, Selector},
};

use super::plain::plain;
use crate::{
  theme::{ExtraColor, ExtraShadow, ExtraSpacing, IconSize},
  ui::tabs::face,
};

/// `Segmented` height, and an icon segment's width (`wsGU4`).
const HEIGHT: f32 = 26.0;
const ICON_SEGMENT_WIDTH: f32 = 28.0;

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<SegmentedControl>(faces(recipe()));
}

fn recipe() -> Recipe {
  use SegmentedControlPart::{Count, Icon, Label, Root, Segment};
  let selected = SegmentedControlState::Selected;
  Recipe::new()
    .base(Root, |style| {
      style
        .height(HEIGHT)
        .padding_x(ExtraSpacing::Tight.role())
        .padding_y(ExtraSpacing::Tight.role())
        .gap(ExtraSpacing::Tight.role())
        .radius(RadiusSize::Md)
        .background(ExtraColor::InputSurface.role())
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    })
    .base(Segment, |style| {
      style
        .height(Length::Fill)
        .padding_x(ExtraSpacing::Loose.role())
        .gap(ExtraSpacing::Snug.role())
        .radius(RadiusSize::Sm)
        .foreground(PaletteColor::MutedForeground)
        .cursor(CursorIcon::Pointer)
    })
    .base(Label, |style| {
      style
        .typography(plain(TypographyStyle::Description))
        .foreground(PaletteColor::MutedForeground)
    })
    .base(Count, |style| {
      style
        .typography(plain(TypographyStyle::Description))
        .foreground(PaletteColor::MutedForeground)
    })
    .base(Icon, |style| style.square(IconSize::Base.size()))
    .hovered(Segment, |style| style.background(ExtraColor::Hover.role()))
    .focus_visible(Segment, |style| style.shadow(ExtraShadow::FocusRing.role()))
    .state(selected, Segment, |style| {
      style
        .background(ExtraColor::Pressed.role())
        .foreground(PaletteColor::Foreground)
    })
    .when(Selector::new().state(selected).hovered(), Segment, |style| {
      style.background(ExtraColor::Pressed.role())
    })
    .state(selected, Label, |style| {
      style
        .typography(plain(TypographyStyle::Label))
        .foreground(PaletteColor::Foreground)
    })
    .state(SegmentedControlState::FullWidth, Root, |style| {
      style.width(Length::Fill)
    })
}

/// An icon segment, a labelled one and one that warns.
fn faces(rules: Recipe) -> Recipe {
  use SegmentedControlPart::{Icon, Label, Segment};
  let selected = SegmentedControlState::Selected;
  rules
    .state(face::ICON, Segment, |style| {
      style.width(ICON_SEGMENT_WIDTH).padding_x(0.0)
    })
    .when(Selector::new().state(face::ICON).state(selected), Segment, |style| {
      style.background(ExtraColor::Hover.role())
    })
    .when(
      Selector::new().state(face::ICON).state(selected).hovered(),
      Segment,
      |style| style.background(ExtraColor::Hover.role()),
    )
    .state(face::WARNED, Segment, |style| style.gap(ExtraSpacing::Pair.role()))
    .state(face::WARNED, Icon, |style| {
      style.square(IconSize::Xs.size()).foreground(PaletteColor::Destructive)
    })
    .when(Selector::new().state(face::WARNED).state(selected), Segment, |style| {
      style.background(PaletteColor::DestructiveMuted)
    })
    .when(
      Selector::new().state(face::WARNED).state(selected).hovered(),
      Segment,
      |style| style.background(PaletteColor::DestructiveMuted),
    )
    .when(Selector::new().state(face::WARNED).state(selected), Label, |style| {
      style.foreground(PaletteColor::Destructive)
    })
}
