//! *Code index use* (ORC-90), a card of *Efficiency* drawn as *Tokens before first edit* (`M94B7Y`) is, since the
//! design has no node of its own for it: every agent invocation that ended, by what it did with the code index
//! (searched the index it got, got it and never searched it, worked without it), each group's median spent tokens as
//! a single-series bar with its invocation count, and the index searches in all. A step that only reads, which
//! *Tokens before first edit* leaves out, shows here.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{IndexGroup, IndexUse};
use orchester_workspace::usage::Dashboard;

use super::{
  Views,
  card::{Card, footnote, push_down},
  efficiency::{COUNT_COLUMN, MEDIAN_COLUMN, median_bar, switch},
  table::{Cell, Density, Width, column, row, table},
  words,
};
use crate::theme::ExtraSpacing;

/// The groups in the card's order: their element ids and names.
const GROUPS: [(&str, &str); 3] = [
  ("usage-index-searched", "usage.efficiency.indexSearched"),
  ("usage-index-unsearched", "usage.efficiency.indexNotSearched"),
  ("usage-index-without", "usage.efficiency.withoutIndex"),
];

pub fn card(ctx: &mut Ctx, dashboard: &Dashboard, views: &Signal<Views>) -> Element {
  let card = Card::new("usage-index", ctx.t("usage.efficiency.indexUse"))
    .small()
    .subtitle(ctx.t("usage.efficiency.indexUseSubtitle"))
    .trailing(switch(ctx, "usage-index-view", views, |views| &mut views.index_table))
    .fill();
  let index = dashboard.report.efficiency.index_use;
  let groups = groups(&index);
  let searches: u64 = groups.iter().map(|group| group.searches).sum();
  let note = footnote(
    "usage-index-footnote",
    &ctx.t_args(
      "usage.efficiency.indexUseNote",
      [("searches", searches.to_string().as_str())],
    ),
  );
  if views.get().index_table {
    return card.body([index_table(ctx, &groups), push_down(), note]);
  }
  let largest = groups.iter().filter_map(|group| group.median).max().unwrap_or(0);
  let bars = GROUPS.iter().zip(groups).enumerate().map(|(at, (&(id, key), group))| {
    let caption = caption(ctx, group, at == 0);
    median_bar(ctx, (id, &ctx.t(key)), (group.median, largest), &caption)
  });
  let bars = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inset.role())
    .with_children(bars);
  card.body([bars.into(), push_down(), note])
}

/// In the order of [`GROUPS`].
const fn groups(index: &IndexUse) -> [IndexGroup; 3] {
  [index.searched, index.not_searched, index.without_index]
}

/// *3 invocations · 41 searches* for the group that searched the index, *3 invocations* for the others.
fn caption(ctx: &Ctx, group: IndexGroup, searched: bool) -> Arc<str> {
  let count = group.invocations.to_string();
  if searched {
    let searches = group.searches.to_string();
    ctx.t_args(
      "usage.efficiency.invocationsSearching",
      [("count", count.as_str()), ("searches", searches.as_str())],
    )
  } else {
    ctx.t_args("usage.efficiency.invocations", [("count", count.as_str())])
  }
}

fn index_table(ctx: &Ctx, groups: &[IndexGroup; 3]) -> Element {
  let columns = [
    column(ctx.t("usage.table.name"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.median"), Width::Px(MEDIAN_COLUMN), true),
    column(ctx.t("usage.table.invocations"), Width::Px(COUNT_COLUMN), true),
  ];
  let rows = GROUPS
    .iter()
    .zip(groups)
    .map(|(&(id, key), group)| {
      let median = group
        .median
        .map_or_else(|| ctx.t("usage.unknown"), |median| words::exact(ctx, median));
      let cells = vec![
        Cell::Text(ctx.t(key)),
        Cell::Text(median),
        Cell::Text(group.invocations.to_string().into()),
      ];
      row(&format!("{id}-row"), &columns, Density::Data, cells).into()
    })
    .collect();
  table(ctx, "usage-index-table", &columns, Density::Data, rows)
}
