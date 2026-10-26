//! A choice of the runs-on picker (`Model` `IAMbH`, `Effort` `VToZ2` in `WaQcn`; `Effort dropdown` `w5O9TT`): a
//! 36-high row, 4 by 8 inside, 8 apart: its label (12 `text_muted`, 56 wide) and its field (28 high on
//! `surface_input` in a `border` stroke, `border_focus` while open, radius 6, 10 at its sides: the chosen option in 12,
//! a model's id in Geist Mono 12 as the CLI takes it (`DBf1P`), and a chevron). Open, its options show under it,
//! inside the picker, as a select menu does (`surface_raised` in a
//! `border_strong` stroke, radius 8, 4 inside, 2 apart): each 28 high, 8 at its sides, its words (12) and what it
//! adds (11 `text_muted`), the chosen one checked on the `hover` fill. They open inside the picker, not over it, so the
//! picker stays the one popup.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Column, Row, Spacer, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ControlHeight, ExtraColor, ExtraRadius, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The label's width (`jD0l4`) and an option's height (`acjKu`).
const LABEL_WIDTH: f32 = 56.0;
const OPTION_HEIGHT: f32 = 28.0;

/// An option: its value, its words and what it adds.
pub(super) struct Choice {
  pub(super) value: String,
  pub(super) label: Arc<str>,
  pub(super) detail: Option<Arc<str>>,
}

/// The choice named `id`, labelled `label`, of `options`, bound to `value`, its options' words in `look`; `open`
/// shows its options.
pub(super) fn select(
  id: &str,
  label: &str,
  (value, open): (&Signal<String>, &Signal<bool>),
  (options, look): (Vec<Choice>, TypographyStyle),
) -> Element {
  let chosen = value.get();
  let shown = open.get();
  let words = options
    .iter()
    .find(|option| option.value == chosen)
    .map_or_else(|| Arc::from(chosen.as_str()), |option| option.label.clone());
  let toggle = open.clone();
  let field = pressable(move || toggle.update(|open| *open = !*open))
    .id(id.to_owned())
    .describe("state", if shown { "open" } else { "closed" })
    .describe("value", chosen.clone())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(
      BorderSize::Sm,
      if shown {
        PaletteColor::BorderFocus
      } else {
        PaletteColor::Border
      },
    ))
    .child(
      Text::new(&words)
        .id(format!("{id}-value"))
        .variant(look)
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(icon(
      if shown { Icon::ChevronUp } else { Icon::ChevronDown },
      IconSize::Md,
      PaletteColor::TextMuted,
    ));
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(label)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(LABEL_WIDTH))
        .flex_shrink(0.0),
    )
    .child(field);
  Column::new()
    .id(format!("{id}-row"))
    .width(Dimension::Pct(100.0))
    .child(row)
    .with_children(shown.then(|| menu(id, (value, open), (options, look), &chosen)))
    .into()
}

/// The options under the field, their words in `look`; choosing one closes them.
fn menu(
  id: &str,
  (value, open): (&Signal<String>, &Signal<bool>),
  (options, look): (Vec<Choice>, TypographyStyle),
  chosen: &str,
) -> Element {
  let rows = options.into_iter().enumerate().map(|(index, option)| {
    let picked = option.value == chosen;
    let (value, open, choice) = (value.clone(), open.clone(), option.value.clone());
    let row = pressable(move || {
      value.set(choice.clone());
      open.set(false);
    })
    .id(format!("{id}-option-{index}"))
    .describe("chosen", picked.to_string())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(OPTION_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(
      Text::new(&option.label)
        .variant(look)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .with_children(
      option
        .detail
        .map(|detail| Text::new(&detail).variant(TypographyStyle::Caption).nowrap()),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(picked.then(|| icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary)));
    if picked {
      row.background(ExtraColor::Hover.role())
    } else {
      row
    }
  });
  Column::new()
    .id(format!("{id}-menu"))
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Xs),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(SpacingSize::Xs)
        .spacing(ExtraSpacing::Tight.role())
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
        .rounded(ExtraRadius::Tile.role())
        .with_children(rows),
    )
    .into()
}
