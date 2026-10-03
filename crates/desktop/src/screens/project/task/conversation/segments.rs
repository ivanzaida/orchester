//! A segmented control in the usage dashboard's period style: segments on the `$sidebar` fill
//! inside a `$border` stroke, the chosen one on `$hover` (the *Show* control of `RrScB`).

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, TypographyStyle},
  components::{Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  theme::{ExtraRadius, ExtraSpacing},
  ui::pressable::{pressable, surface},
};

/// Segment height (`RrScB`).
const SEGMENT_HEIGHT: f32 = 26.0;

/// The segments `options` of `value`, the chosen one marked; `id` names the control and, with each
/// option's index, its segments.
pub(super) fn segments<T>(id: &'static str, value: &Signal<T>, options: Vec<(T, Arc<str>)>) -> Element
where
  T: Clone + PartialEq + Send + Sync + 'static + lurq::core::signal::SignalValue,
{
  let chosen = value.get();
  let segments = options.into_iter().enumerate().map(|(index, (option, label))| {
    let selected = option == chosen;
    let text = Text::new(&label)
      .variant(TypographyStyle::Button)
      .nowrap()
      .color(if selected {
        PaletteColor::TextPrimary
      } else {
        PaletteColor::TextSecondary
      });
    let segment = Row::new()
      .height(Dimension::Px(SEGMENT_HEIGHT))
      .padding_horizontal(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(ExtraRadius::Segment.role())
      .describe("selected", selected.to_string())
      .child(text);
    let segment = if selected {
      segment.background(PaletteColor::AccentMuted)
    } else {
      segment
    };
    let value = value.clone();
    Element::from(pressable(surface().id(format!("{id}-{index}")), move || value.set(option.clone())).child(segment))
  });
  Row::new()
    .id(id)
    .padding(ExtraSpacing::Hair.role())
    .spacing(ExtraSpacing::Hair.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .with_children(segments)
    .into()
}
