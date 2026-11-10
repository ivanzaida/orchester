//! The compact picker's Search and model list (`Search` `V6dY4P`, `Models` `BYfVi` in `ZcGGy`; K2 `rjPcV`). Search is a
//! 30-high strip between two `border` lines, 10 at its sides and its parts 8 apart: a 13 glass, what is typed in 12
//! `text_primary` (*Search models…* in `text_muted`) and, once it holds text, a 13 cross that clears it. The models are
//! in the app's font, 12 `text_primary`, rows 28 high with radius `sm`, 8 at their sides; the chosen one has a 13 check
//! at its end and nothing else, the one the keys are on the `hover` fill. The list is always four rows high in 4 of
//! padding (120), so the picker never changes height: more rows scroll inside it under a 3-wide scrollbar, and fewer
//! (while searching, say) leave the rest empty.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::{TextInputEvent, mouse_event::MouseEvent},
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, ScrollVertical, Text, TextInput, TextOverflow},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::ScrollState},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::model::{LIST_ROWS, Model};
use crate::{
  theme::{ControlHeight, ExtraColor, ExtraSpacing, IconSize, runs_on_picker_scrollbar},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The list's padding (`BYfVi`): 4 on every side.
const LIST_PADDING: f32 = 4.0;

/// The rows' height (`BnuiI`) and the list's inner height.
fn row_height() -> f32 {
  ControlHeight::Md.value()
}

fn viewport() -> f32 {
  // Four rows: f32 is exact.
  row_height() * LIST_ROWS as f32
}

/// The list's height with its padding: the picker never grows past it nor shrinks under it.
pub fn list_height() -> f32 {
  viewport() + LIST_PADDING * 2.0
}

/// Scrolls the list held by `scroll` so that its `index`th row shows.
pub(super) fn reveal(scroll: &ScrollState, index: usize) {
  // A list of a few dozen rows: f32 is precise enough.
  let top = index as f32 * row_height();
  let bottom = top + row_height();
  let shown = scroll.scroll_y();
  if top < shown {
    scroll.set_scroll_pending(0.0, top);
  } else if bottom > shown + viewport() {
    scroll.set_scroll_pending(0.0, bottom - viewport());
  }
}

/// What Search holds, the field that takes the focus and what its typing does.
pub(super) struct SearchBox {
  pub(super) query: Signal<String>,
  pub(super) field: ElementRef,
  pub(super) typed: Arc<dyn Fn() + Send + Sync>,
}

/// Search, named `id`.
pub(super) fn search(ctx: &mut Ctx, id: &str, search: &SearchBox) -> Element {
  let theme = ctx.theme();
  let typed_style = FormTextRole {
    typography: TypographyStyle::Description,
    color: PaletteColor::TextPrimary,
  }
  .resolve(&theme.typography(), &theme.palette());
  let placeholder_style = FormTextRole {
    typography: TypographyStyle::Description,
    color: PaletteColor::TextMuted,
  }
  .resolve(&theme.typography(), &theme.palette());
  let words = ctx.t("ui.runsOnPicker.search");
  let typed = search.typed.clone();
  let input = TextInput::styled(search.query.clone(), typed_style)
    .placeholder_style(placeholder_style)
    .caret_color(PaletteColor::TextPrimary)
    .id(format!("{id}-search"))
    .describe("a11y_name", words.clone())
    .placeholder(&words)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .ref_element(search.field.clone())
    .on_input(move |_: TextInputEvent| typed())
    .single_line();
  let clear = (!search.query.get().is_empty()).then(|| {
    let query = search.query.clone();
    let typed = search.typed.clone();
    Button::empty()
      .id(format!("{id}-search-clear"))
      .align_items(Alignment::Center)
      .focusable(false)
      .cursor(CursorIcon::Pointer)
      .describe("a11y_name", ctx.t("ui.runsOnPicker.clear"))
      .on_click(move |event: MouseEvent| {
        event.stop_propagation();
        query.set(String::new());
        typed();
      })
      .child(icon(Icon::X, IconSize::Base, PaletteColor::TextMuted))
  });
  let line = || Border::inside(BorderSize::Sm, PaletteColor::Border);
  Row::new()
    .id(format!("{id}-search-box"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Row.value()))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(line())
    .border_bottom(line())
    .child(icon(Icon::Search, IconSize::Base, PaletteColor::TextMuted))
    .child(input)
    .with_children(clear)
    .into()
}

/// The list of `models` (as Search filtered them), the chosen one checked and the keys at `cursor`; `choose` takes the
/// model at a place in `models`. With none, `empty` says why.
pub(super) fn list(
  ctx: &mut Ctx,
  (id, scroll): (&str, &ScrollState),
  (models, empty): (&[&Model], &str),
  (chosen, cursor): (Option<&str>, Option<usize>),
  choose: &Arc<dyn Fn(usize) + Send + Sync>,
) -> Element {
  let rows: Vec<Element> = if models.is_empty() {
    vec![nothing(id, empty)]
  } else {
    models
      .iter()
      .enumerate()
      .map(|(index, model)| {
        let press = choose.clone();
        row(
          (id, index, &model.id),
          (chosen == Some(&*model.id), cursor == Some(index)),
          move || press(index),
        )
      })
      .collect()
  };
  Column::new()
    .id(format!("{id}-models"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(list_height()))
    .flex_shrink(0.0)
    .padding(LIST_PADDING)
    .child(
      ScrollVertical::new(Column::new().width(Dimension::Pct(100.0)).with_children(rows))
        .with_scroll_state(scroll.clone())
        .id(format!("{id}-model-scroll"))
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(viewport()))
        .scrollbar(runs_on_picker_scrollbar(&ctx.theme().scrollbar())),
    )
    .into()
}

/// A model's row: its id in the app's font, a check at its end when `chosen`, the `hover` fill where the keys are
/// (`current`).
fn row(
  (id, index, model): (&str, usize, &str),
  (chosen, current): (bool, bool),
  press: impl Fn() + Send + Sync + 'static,
) -> Element {
  let row = pressable(press)
    .id(format!("{id}-model-{index}"))
    .describe("model", model.to_owned())
    .describe("chosen", chosen.to_string())
    .describe("current", current.to_string())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(row_height()))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(
      Text::new(model)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .with_children(chosen.then(|| icon(Icon::Check, IconSize::Base, PaletteColor::TextPrimary)));
  if current {
    row.background(ExtraColor::Hover.role()).into()
  } else {
    row.into()
  }
}

/// Why the list has no row.
fn nothing(id: &str, words: &str) -> Element {
  Row::new()
    .id(format!("{id}-no-models"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(row_height()))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .align_items(Alignment::Center)
    .child(Text::new(words).variant(TypographyStyle::Description).nowrap())
    .into()
}
