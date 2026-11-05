//! The runs-on picker's Models column (`Models` `iIhx1` in `v6mWp8`; `Searching` `b187v` and `Many models` `q1ZHoz` in
//! `Runs-on picker · states` `t7cdi`): 8 inside, its parts 6 apart, it fills the room between the rail and Effort. Its
//! head is the connection shown ([`super::header`]), then Search (30 high on `surface_input` in a `border` stroke,
//! `border_focus` while focused, radius 6, a 13 glass; *Search models…*, on an API provider *Search or type a model…*,
//! in 12 `text_muted`, what is typed in Geist Mono 12), then the models 2 apart, each its id in Geist Mono 12 as the
//! CLI takes it, the chosen one checked. At most seven show (222 high, `GaA7N`): the rest scroll inside the column, 8
//! kept at the list's end for a 3-wide `border_strong` scrollbar, so the panel keeps its height; the chosen model is
//! scrolled into view when the picker opens, and the keys' place as they move. Search filters them; nothing found says
//! so; on an API provider a model it does not list can be typed and taken with *Use …* or Enter.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ConnectionId, resource::ModelId};
use orchester_workspace::orchestrator::settings::OrchestratorSetup;

use super::{
  item::item,
  picking::{Chooser, Picking},
};
use crate::{
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, picker_scrollbar},
  ui::field::FieldInput,
};

/// How many places the list shows at most, and its height then (`GaA7N`): seven 30-high rows 2 apart.
const SHOWN: usize = 7;
const LIST_HEIGHT: f32 = 222.0;

/// A place in the models' list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ModelItem {
  /// A model the connection lists.
  Listed(ModelId),
  /// A model typed in Search that an API provider's list does not have.
  Typed(ModelId),
}

/// The places of `connection`'s list as Search (`query`) filters it, `chosen` among its models; `typed` takes a model
/// Search names that the list does not have.
pub(super) fn items(
  setup: &OrchestratorSetup,
  (connection, chosen): (ConnectionId, Option<&ModelId>),
  (query, typed): (&str, bool),
) -> Vec<ModelItem> {
  let models: Vec<ModelId> = setup
    .models(connection, chosen)
    .into_iter()
    .map(|option| option.id)
    .collect();
  let wanted = query.trim().to_lowercase();
  let named = ModelId::new(query.trim())
    .ok()
    .filter(|named| typed && !wanted.is_empty() && !models.contains(named));
  models
    .into_iter()
    .filter(|model| model.as_str().to_lowercase().contains(&wanted))
    .map(ModelItem::Listed)
    .chain(named.map(ModelItem::Typed))
    .collect()
}

/// Scrolls the list held by `scroll` so that its `index`th place shows.
pub(super) fn reveal(scroll: &ScrollState, index: usize) {
  let pitch = ControlHeight::Row.value() + ExtraSpacing::Tight.value();
  // A list of a few dozen places: f32 is precise enough.
  let top = index as f32 * pitch;
  let bottom = top + ControlHeight::Row.value();
  let shown = scroll.scroll_y();
  if top < shown {
    scroll.set_scroll_pending(0.0, top);
  } else if bottom > shown + LIST_HEIGHT {
    scroll.set_scroll_pending(0.0, bottom - LIST_HEIGHT);
  }
}

/// The column: `header`, Search and `list`, the chosen model checked and the keys at `cursor`.
pub(super) fn column(
  ctx: &mut Ctx,
  (header, list): (Element, &[ModelItem]),
  (chosen, cursor, typed): (Option<&ModelId>, Option<usize>, bool),
  chooser: &Chooser,
) -> Element {
  let search = search(ctx, &chooser.picking, typed);
  let rows: Vec<Element> = if list.is_empty() {
    vec![nothing_found(ctx)]
  } else {
    list
      .iter()
      .enumerate()
      .map(|(index, place)| row(ctx, (index, place), (chosen, cursor == Some(index)), chooser))
      .collect()
  };
  Column::new()
    .id("orchestrator-runs-on-models")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .align(Alignment::Stretch)
    .padding(SpacingSize::Sm)
    .spacing(ExtraSpacing::Snug.role())
    .child(header)
    .child(search)
    .child(
      ScrollVertical::new(places(rows, list.len() > SHOWN))
        .with_scroll_state(chooser.picking.scroll.clone())
        .id("orchestrator-runs-on-model-scroll")
        .width(Dimension::Pct(100.0))
        .max_height(Dimension::Px(LIST_HEIGHT))
        .flex_shrink(1.0)
        .min_height(Dimension::Px(0.0))
        .scrollbar(picker_scrollbar(&ctx.theme().scrollbar())),
    )
    .into()
}

/// The list's `rows`, 8 kept at their end for the scrollbar when they `scroll`.
fn places(rows: Vec<Element>, scroll: bool) -> Column {
  let list = Column::new()
    .id("orchestrator-runs-on-model-list")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(rows);
  if scroll {
    list.padding(Padding::new().right(SpacingSize::Sm))
  } else {
    list
  }
}

/// Search, focused when the picker asks; `typed` names its other placeholder.
fn search(ctx: &mut Ctx, picking: &Picking, typed: bool) -> Element {
  let placeholder = if typed {
    ctx.t("orchestrator.runsOn.searchOrType")
  } else {
    ctx.t("orchestrator.runsOn.search")
  };
  let typing = picking.clone();
  FieldInput::new(
    "orchestrator-runs-on-search",
    placeholder.clone(),
    picking.query.clone(),
  )
  .search()
  .text_style(ExtraTypography::PropertyMono.role())
  .placeholder_style(TypographyStyle::Description)
  .placeholder(placeholder)
  .box_height(ControlHeight::Row.value())
  .element(picking.search.clone())
  .on_typed(move |_| typing.typed())
  .render(ctx)
}

/// A place of the list, the `index`th, `chosen` checked and where the keys are (`current`).
fn row(
  ctx: &Ctx,
  (index, place): (usize, &ModelItem),
  (chosen, current): (Option<&ModelId>, bool),
  chooser: &Chooser,
) -> Element {
  let chooser = chooser.clone();
  match place {
    ModelItem::Listed(model) => {
      let label = model_label(model.as_str());
      let picked = chosen == Some(model);
      let model = model.clone();
      item(
        format!("orchestrator-runs-on-model-{index}"),
        [label],
        (picked, current),
        move || chooser.model(model.clone()),
      )
    }
    ModelItem::Typed(model) => {
      let words = ctx.t_args("orchestrator.runsOn.useTyped", [("model", model.as_str())]);
      let label = Text::new(&words)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis);
      let model = model.clone();
      item(
        "orchestrator-runs-on-typed".to_owned(),
        [label.into()],
        (false, current),
        move || chooser.model(model.clone()),
      )
    }
  }
}

/// A model's id as the CLI takes it, never a prettified name.
fn model_label(model: &str) -> Element {
  Text::new(model)
    .variant(ExtraTypography::PropertyMono.role())
    .color(PaletteColor::TextPrimary)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .into()
}

/// That Search found nothing.
fn nothing_found(ctx: &Ctx) -> Element {
  Row::new()
    .id("orchestrator-runs-on-no-match")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Row.value()))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("orchestrator.runsOn.noMatch")).variant(TypographyStyle::Description))
    .into()
}
