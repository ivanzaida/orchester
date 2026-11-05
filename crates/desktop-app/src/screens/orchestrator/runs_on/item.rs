//! A choice in the runs-on picker's Models and Effort columns (`Item` `geKVF`, `Item/chosen` `qSMqB`, `Item/hover`
//! `VlFmy` in `v6mWp8`): 30 high, radius 4, 8 at its sides, its parts 8 apart and a 14 check at its end on the chosen
//! one; on the `hover` fill under the pointer and where the keys are.

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize},
  components::Button,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ControlHeight, ExtraColor, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The choice named `id` with `parts`, `chosen` and where the keys are (`current`) as said; `press` chooses it.
pub(super) fn item(
  id: String,
  parts: impl IntoIterator<Item = Element>,
  (chosen, current): (bool, bool),
  press: impl Fn() + Send + Sync + 'static,
) -> Element {
  let row = pressable(press)
    .id(id)
    .describe("chosen", chosen.to_string())
    .describe("current", current.to_string())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Row.value()))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .with_children(parts)
    .with_children(chosen.then(|| icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary)));
  let row: Button = if current {
    row.background(ExtraColor::Hover.role())
  } else {
    row
  };
  row.into()
}
