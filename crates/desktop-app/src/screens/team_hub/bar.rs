//! The pinned actions of every panel of the hub (`Actions` `x3uhYJ` in `t4mtQL`, `o8tFHR` in `NDZXC`, `WpN6M` in
//! `SpRvd`, `hDhr8` in `c3jU1R`): the buttons of the fix of the first missing part at its start and,
//! at its end, what the person is told about it.

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize},
  components::{Row, Text},
  layout::{Alignment, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::theme::{HubSpacing, HubText};

/// The bar with `buttons` at its start and `hint` at its end; the kind's panels each say what they put in them.
pub(in crate::screens) fn bar(buttons: Vec<Element>, hint: Option<String>) -> Element {
  bar_saying(buttons, hint.map(|hint| (hint, PaletteColor::TextMuted)))
}

/// [`bar`] with `hint` in `color`: the form that creates a resource says in `danger` why Create cannot be pressed.
pub(in crate::screens) fn bar_saying(buttons: Vec<Element>, hint: Option<(String, PaletteColor)>) -> Element {
  let bar = Row::new()
    .id("team-hub-actions")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HubSpacing::Actions.value()))
    .padding(Padding::horizontal(HubSpacing::Sections.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(buttons);
  // The hint takes what the buttons leave and wraps, so a long one is never cut at the panel's edge.
  match hint {
    Some((hint, color)) => bar.child(
      Text::new(&hint)
        .id("team-hub-actions-hint")
        .variant(HubText::PanelHint.role())
        .color(color)
        .text_align(TextAlign::Right)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    ),
    None => bar.child(Row::new().flex(1.0)),
  }
  .into()
}
