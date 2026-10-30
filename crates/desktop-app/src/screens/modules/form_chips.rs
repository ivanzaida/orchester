//! The chips of New module and Edit module (`Properties` `MyYPA` in `08`, `eJj5w` in `09`, `J0390O` in `eyrnI`), the
//! task form's property chips (`task_form::chips`), 8 apart and 4 under the description: State, set, for a module that
//! exists, opening a menu of Open and Closed; Start date and Target date, each saying its name and its day
//! (`calendar`, `calendar-check`), opening the date picker under it with the other date ruling its range and tinted on
//! it; and Base branch after them (`super::form_base`).

use std::sync::Arc;

use chrono::NaiveDate;
use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{module::ModuleState, time::Timestamp, usage::local_date};

use super::{
  form::{Day, Fields, FormProps, StatePick},
  form_base::{self, BaseChip},
  words,
};
use crate::{
  screens::{
    task_form::chips::{ChipState, Look, PropertyChip, chip_row},
    usage::words::day,
  },
  ui::{
    date_picker::{Limits, PickerProps, date_panel},
    filter::filter_menu_panel,
    icon::Icon,
    menu::MenuItem,
  },
};

/// The State menu's width.
const MENU_WIDTH: f32 = 160.0;

/// The chips' popovers.
#[derive(Clone)]
pub struct Chips {
  state: ChipState,
  start: ChipState,
  target: ChipState,
  base: BaseChip,
}

impl Chips {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      state: ChipState::new(ctx),
      start: ChipState::new(ctx),
      target: ChipState::new(ctx),
      base: BaseChip::new(ctx),
    }
  }
}

pub fn row(ctx: &mut Ctx, props: &FormProps, fields: &Fields, chips: &Chips) -> Element {
  let mut row = Vec::new();
  if props.module.is_some() {
    row.push(state_chip(ctx, &fields.state, &chips.state));
  }
  let (start, target) = (fields.start.get().0, fields.target.get().0);
  row.push(date_chip(
    ctx,
    ("module-start", "modules.form.start", Icon::Calendar),
    (
      &fields.start,
      target,
      Limits {
        earliest: None,
        latest: target,
      },
    ),
    &chips.start,
  ));
  row.push(date_chip(
    ctx,
    ("module-target", "modules.form.target", Icon::CalendarCheck),
    (
      &fields.target,
      start,
      Limits {
        earliest: start,
        latest: None,
      },
    ),
    &chips.target,
  ));
  row.push(form_base::chip(ctx, props, &fields.base, &chips.base));
  Column::new()
    .id("module-form-properties")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Xs))
    .child(chip_row("module-form-chips", row))
    .into()
}

fn state_chip(ctx: &mut Ctx, chosen: &Signal<StatePick>, state: &ChipState) -> Element {
  let current = chosen.get().0;
  let panel = state.is_open().then(|| {
    let rows: Vec<Element> = [ModuleState::Open, ModuleState::Closed]
      .into_iter()
      .map(|choice| {
        let chosen = chosen.clone();
        let id = match choice {
          ModuleState::Open => "module-state-open",
          ModuleState::Closed => "module-state-closed",
        };
        MenuItem::new(id, words::state(ctx, choice), move || chosen.set(StatePick(choice)))
          .in_form()
          .chosen(choice == current)
          .render(ctx, &state.open)
      })
      .collect();
    filter_menu_panel("module-state-menu", MENU_WIDTH)
      .with_children(rows)
      .into()
  });
  PropertyChip::new(
    "module-state",
    Icon::CircleDot,
    (ctx.t("modules.form.state"), words::state(ctx, current)),
    Look::Set,
  )
  .render(state, panel)
}

/// A date's chip: its name and its day, the picker under it while it is open.
fn date_chip(
  ctx: &mut Ctx,
  (id, key, glyph): (&str, &str, Icon),
  (value, other, limits): (&Signal<Day>, Option<NaiveDate>, Limits),
  state: &ChipState,
) -> Element {
  let chosen = value.get().0;
  let words = chosen.map_or_else(|| ctx.t("modules.form.noDate"), |date| day(ctx, date));
  let value = value.clone();
  let panel = state.is_open().then(|| {
    let picker = PickerProps {
      id: format!("{id}-picker").into(),
      chosen,
      other,
      limits,
      today: local_date(Timestamp::now()),
      open: state.open.clone(),
      pick: Arc::new(move |day| value.set(Day(day))),
    };
    date_panel(ctx, picker)
  });
  PropertyChip::new(id, glyph, (ctx.t(key), words), Look::Empty).render(state, panel)
}
