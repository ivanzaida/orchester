//! A segmented control in the usage dashboard's period style: segments on the `$sidebar` fill
//! inside a `$border` stroke, the chosen one on `$hover`. The Usage dashboard's period control is 36
//! high (`C2pTYF`); its switches and the conversation's *Show* control (`RrScB`) are compact.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, TypographyStyle},
  components::{Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  theme::{ExtraRadius, ExtraSpacing},
  ui::pressable::{pressable, surface},
};

/// Segment heights: the compact switches (`RrScB`) and the period control, 36 with its padding and
/// stroke (`C2pTYF`).
const COMPACT_HEIGHT: f32 = 26.0;
const LARGE_HEIGHT: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentSize {
  Compact,
  Large,
}

/// The segments `options`, `chosen` marked; a press on another one hands it to `choose`. `id` names
/// the control and, with each option's index, its segments.
pub fn segments<T>(
  id: &'static str,
  chosen: &T,
  options: Vec<(T, Arc<str>)>,
  size: SegmentSize,
  choose: impl Fn(T) + Clone + Send + Sync + 'static,
) -> Element
where
  T: Clone + PartialEq + Send + Sync + 'static,
{
  let height = match size {
    SegmentSize::Compact => COMPACT_HEIGHT,
    SegmentSize::Large => LARGE_HEIGHT,
  };
  let segments = options.into_iter().enumerate().map(|(index, (option, label))| {
    let selected = option == *chosen;
    let text = Text::new(&label)
      .variant(TypographyStyle::Button)
      .nowrap()
      .color(if selected {
        PaletteColor::TextPrimary
      } else {
        PaletteColor::TextSecondary
      });
    let segment = Row::new()
      .height(Dimension::Px(height))
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
    let choose = choose.clone();
    Element::from(pressable(surface().id(format!("{id}-{index}")), move || choose(option.clone())).child(segment))
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
