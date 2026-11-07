//! The pinned actions of every panel of the hub (`Actions` `x3uhYJ` in `t4mtQL`, `o8tFHR` in `NDZXC`, `WpN6M` in
//! `SpRvd`, `hDhr8` in `c3jU1R`): the buttons of the fix of the first missing part (or the admin's *Edit*) at its
//! start and, at its end, what the person is told about it.

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::theme::{HubSpacing, HubText};

/// The bar with `buttons` at its start and `hint` at its end; the kind's panels each say what they put in them.
pub(super) fn bar(buttons: Vec<Element>, hint: Option<String>) -> Element {
  Row::new()
    .id("team-hub-actions")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HubSpacing::Actions.value()))
    .padding(Padding::horizontal(HubSpacing::Sections.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(buttons)
    .child(Row::new().flex(1.0))
    .with_children(hint.map(|hint| {
      Text::new(&hint)
        .id("team-hub-actions-hint")
        .variant(HubText::Hint.role())
        .nowrap()
    }))
    .into()
}
