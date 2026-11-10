//! The compact picker's Effort row (`Effort` `nj6o0`, `Segmented` `LDUge` in `ZcGGy`): one segmented row at the bottom
//! under a `border` line, 6 inside, 28 high on `surface_input` with radius `md`, 2 inside and between its segments:
//! *Default* and the efforts the chosen model offers, each segment sharing the width, 24 high, radius `sm`, 11.5
//! `text_muted`; the chosen one on `pressed` in 11.5/500 `text_primary`. With no model chosen on the connection shown
//! the row keeps its place with *Default* alone, at 40 % and inert.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Button, Column, Row, Text},
  layout::layout_kind::Justify,
  node::{Element, Style, border::Border, dimension::Dimension},
};

use crate::{
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, RunsOnPickerText},
  ui::pressable::{inert, pressable},
};

/// The words of an effort as the CLI names it; one with no word of its own shows as the CLI names it.
pub fn effort_word(ctx: &Ctx, effort: &str) -> Arc<str> {
  match effort {
    "minimal" => ctx.t("ui.runsOnPicker.effort.minimal"),
    "low" => ctx.t("ui.runsOnPicker.effort.low"),
    "medium" => ctx.t("ui.runsOnPicker.effort.medium"),
    "high" => ctx.t("ui.runsOnPicker.effort.high"),
    "xhigh" => ctx.t("ui.runsOnPicker.effort.xhigh"),
    "max" => ctx.t("ui.runsOnPicker.effort.max"),
    other => other.into(),
  }
}

/// The row for `efforts` (what the chosen model offers, `None` while there is no model chosen), `chosen` marked (`None`
/// is *Default*); `choose` takes an effort.
pub(super) fn row(
  ctx: &Ctx,
  id: &str,
  (efforts, chosen): (Option<&[Arc<str>]>, Option<&str>),
  choose: &Arc<dyn Fn(Option<Arc<str>>) + Send + Sync>,
) -> Element {
  let offered = efforts.unwrap_or_default();
  let places = std::iter::once(None).chain(offered.iter().map(Some));
  let segments: Vec<Element> = places
    .map(|effort| {
      let words = effort.map_or_else(
        || ctx.t("ui.runsOnPicker.effort.default"),
        |effort| effort_word(ctx, effort),
      );
      let name = effort.map_or("default", |effort| &**effort);
      let value = effort.cloned();
      let picked = efforts.is_some() && value.as_deref() == chosen;
      segment((id, name, &words), (efforts.is_some(), picked), {
        let choose = choose.clone();
        move || choose(value.clone())
      })
    })
    .collect();
  let segmented = Row::new()
    .id(format!("{id}-effort-row"))
    .describe("a11y_name", ctx.t("ui.runsOnPicker.effort.label"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(ExtraSpacing::Tight.role())
    .spacing(ExtraSpacing::Tight.role())
    .background(PaletteColor::SurfaceInput)
    .rounded(RadiusSize::Md)
    .with_children(segments);
  let segmented = if efforts.is_some() {
    segmented
  } else {
    segmented.opacity(DISABLED_OPACITY)
  };
  Column::new()
    .id(format!("{id}-effort"))
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Snug.role())
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(segmented)
    .into()
}

/// A segment named `name` saying `words`, `picked` or not; `active` when it can be pressed.
fn segment(
  (id, name, words): (&str, &str, &str),
  (active, picked): (bool, bool),
  press: impl Fn() + Send + Sync + 'static,
) -> Element {
  let button = if active {
    pressable(press)
  } else {
    inert(Button::empty())
  };
  let (typography, fill) = if picked {
    (RunsOnPickerText::SegmentChosen, Some(ExtraColor::Pressed.role()))
  } else {
    (RunsOnPickerText::Segment, None)
  };
  let button = button
    .id(format!("{id}-effort-{name}"))
    .describe("chosen", picked.to_string())
    .describe("a11y_name", words.to_owned())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Px(ControlHeight::Sm.value()))
    .justify(Justify::Center)
    .rounded(RadiusSize::Sm)
    .child(Text::new(words).variant(typography.role()).nowrap());
  match fill {
    Some(fill) => button.background(fill).into(),
    None if active => button
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .into(),
    None => button.into(),
  }
}
