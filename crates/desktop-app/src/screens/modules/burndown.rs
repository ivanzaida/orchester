//! A module's *Burndown* card, a Usage card (`screens::usage::card`) with its *Chart* / *Table* switch: its title,
//! what it counts in, the legend (remaining, ideal, added, removed), and the chart ([`super::chart`]) or the same
//! days as a table with exact figures. Without a target date the card says how to get the ideal line.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::Element,
};
use orchester_workspace::modules::{burndown::BurndownDay, data::ModulePage};

use super::{
  chart::{BurndownChart, ChartDay, ChartProps},
  words,
};
use crate::{
  screens::usage::{
    card::{Card, footnote, swatch, view_switch},
    plot::{AxisName, label_every},
    table::{Cell, Density, Width, column, row, table},
    words::{long_day, weekday},
  },
  theme::{ExtraColor, ExtraSpacing},
};

/// Day labels the chart keeps, at most.
const DAY_LABELS: f32 = 10.0;
/// A figure column of the table.
const FIGURE_COLUMN: f32 = 96.0;

pub fn card(ctx: &mut Ctx, page: &ModulePage, table_view: &Signal<bool>) -> Element {
  let tabled = table_view.get();
  let switch = {
    let table_view = table_view.clone();
    view_switch(ctx, "module-burndown-view", tabled, false, move |table| {
      table_view.set(table)
    })
  };
  let card = Card::new("module-burndown", ctx.t("modules.burndown.title"))
    .note(words::unit(ctx, page.burndown.unit))
    .subtitle(ctx.t("modules.burndown.subtitle"))
    .trailing(switch);
  let note = page
    .module
    .details()
    .target()
    .is_none()
    .then(|| footnote("module-burndown-no-target", &ctx.t("modules.burndown.noTarget")));
  let body = if tabled {
    days_table(ctx, &page.burndown.days)
  } else {
    let chart = chart(ctx, &page.burndown.days);
    lurq::components::Column::new()
      .spacing(SpacingSize::Sm)
      .child(legend(ctx))
      .child(chart)
      .into()
  };
  card.body(std::iter::once(body).chain(note))
}

fn legend(ctx: &Ctx) -> Element {
  let entry = |id: &str, color: PaletteColor, key: &str| {
    Row::new()
      .id(id.to_owned())
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(swatch(color))
      .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption).nowrap())
  };
  Row::new()
    .id("module-burndown-legend")
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(entry(
      "module-legend-remaining",
      ExtraColor::Chart1.role(),
      "modules.burndown.remaining",
    ))
    .child(entry(
      "module-legend-ideal",
      PaletteColor::TextMuted,
      "modules.burndown.ideal",
    ))
    .child(entry(
      "module-legend-added",
      ExtraColor::Chart3.role(),
      "modules.burndown.added",
    ))
    .child(entry(
      "module-legend-removed",
      ExtraColor::Chart2.role(),
      "modules.burndown.removed",
    ))
    .into()
}

fn chart(ctx: &mut Ctx, days: &[BurndownDay]) -> Element {
  let every = label_every(days.len(), DAY_LABELS, 1.0);
  let last = days.len().saturating_sub(1);
  let shown: Arc<[ChartDay]> = days
    .iter()
    .enumerate()
    .map(|(index, day)| ChartDay {
      // Counted back from the last day, so it always has its label, as the Usage charts do.
      label: (last - index)
        .is_multiple_of(every)
        .then(|| AxisName::plain(weekday(ctx, day.date))),
      remaining: day.remaining,
      ideal: day.ideal,
      added: day.added,
      removed: day.removed,
      name: long_day(ctx, day.date),
      value: day.remaining.map_or_else(
        || ctx.t("modules.burndown.ahead"),
        |left| {
          ctx.t_args(
            "modules.burndown.pointValue",
            [("remaining", left.to_string().as_str())],
          )
        },
      ),
    })
    .collect();
  ctx.mount_keyed::<BurndownChart>("module-burndown-line", ChartProps { days: shown })
}

/// The chart's days with their exact figures; a day still to come has none but its ideal.
fn days_table(ctx: &Ctx, days: &[BurndownDay]) -> Element {
  let columns = [
    column(ctx.t("modules.table.day"), Width::Flex(1.0), false),
    column(ctx.t("modules.table.scope"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("modules.table.remaining"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("modules.table.ideal"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("modules.table.added"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("modules.table.removed"), Width::Px(FIGURE_COLUMN), true),
  ];
  let ahead = ctx.t("modules.burndown.ahead");
  let figure = |value: Option<u32>| value.map_or_else(|| ahead.clone(), |value| Arc::from(value.to_string()));
  let rows = days
    .iter()
    .enumerate()
    .map(|(index, day)| {
      let past = day.remaining.is_some();
      let cells = vec![
        Cell::Text(long_day(ctx, day.date)),
        Cell::Text(figure(day.scope)),
        Cell::Text(figure(day.remaining)),
        Cell::Muted(
          day
            .ideal
            .map_or_else(|| ahead.clone(), |ideal| Arc::from(format!("{ideal:.1}"))),
        ),
        Cell::Text(figure(past.then_some(day.added))),
        Cell::Text(figure(past.then_some(day.removed))),
      ];
      row(&format!("module-burndown-row-{index}"), &columns, Density::Data, cells).into()
    })
    .collect();
  table(ctx, "module-burndown-table", &columns, Density::Data, rows)
}
