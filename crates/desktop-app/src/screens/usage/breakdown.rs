//! *Where tokens go* (`Card · Where tokens go` `XQRop`): columns of horizontal bars, stacked by CLI and
//! scaled to the column's largest row, each with its spent tokens and share — *By agent profile · step*,
//! *By model* (in mono) and *By pipeline* (`Breakdown row` `YAqCn`). Selecting a row adds its profile, model
//! or pipeline to the filters. Auto mode's reviewer has rows of its own among the profiles, its model in mono
//! then *auto mode reviewer* (`Breakdown row/reviewer` `iSR3L`); it is no profile, so its rows only read, and
//! a line under the title says so while they are shown. As a table, each column lists its rows with exact
//! numbers (`Table` `wnktr`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::usage::{BreakdownKey, BreakdownRow, Breakdowns, ProviderSeries, percent};
use orchester_workspace::usage::Dashboard;

use super::{
  Requery, Views,
  card::{Card, view_switch},
  series::series_fill,
  table::{Cell, Density, Width, column, fit, row, table},
  words,
};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, UsageRadius},
  ui::pressable::pressable,
};

/// Rows a column shows as bars; the table lists them all.
const ROWS: usize = 6;
/// A row's bar (`Bar` `R0DLP5`), and its share's column (`Share` `A6mtEG`).
const BAR_HEIGHT: f32 = 6.0;
const SHARE_WIDTH: f32 = 30.0;
/// The table's figures (`wnktr`).
const SPENT_COLUMN: f32 = 96.0;
const TABLE_SHARE_COLUMN: f32 = 40.0;

/// One of the card's columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Breakdown {
  /// Every project's page only.
  Projects,
  Profiles,
  Models,
  Pipelines,
}

impl Breakdown {
  const ALL: [Self; 4] = [Self::Projects, Self::Profiles, Self::Models, Self::Pipelines];

  const fn title_key(self) -> &'static str {
    match self {
      Self::Projects => "usage.breakdown.byProject",
      Self::Profiles => "usage.breakdown.byProfile",
      Self::Models => "usage.breakdown.byModel",
      Self::Pipelines => "usage.breakdown.byPipeline",
    }
  }

  const fn id(self) -> &'static str {
    match self {
      Self::Projects => "usage-breakdown-projects",
      Self::Profiles => "usage-breakdown-profiles",
      Self::Models => "usage-breakdown-models",
      Self::Pipelines => "usage-breakdown-pipelines",
    }
  }

  fn rows(self, breakdowns: &Breakdowns) -> &[BreakdownRow] {
    match self {
      Self::Projects => &breakdowns.projects,
      Self::Profiles => &breakdowns.profiles,
      Self::Models => &breakdowns.models,
      Self::Pipelines => &breakdowns.pipelines,
    }
  }
}

pub fn card(ctx: &mut Ctx, dashboard: &Arc<Dashboard>, views: &Signal<Views>, requery: &Requery) -> Element {
  let table_shown = views.get().breakdown_table;
  let switch = {
    let views = views.clone();
    view_switch(ctx, "usage-breakdown-view", table_shown, false, move |table| {
      views.update(|views| views.breakdown_table = table);
    })
  };
  let breakdowns = &dashboard.report.breakdowns;
  let spent = dashboard.report.summary.spent();
  let everywhere = dashboard.project.is_none();
  let shown_columns = Breakdown::ALL
    .into_iter()
    .filter(|shown| everywhere || *shown != Breakdown::Projects);
  let columns = shown_columns.map(|shown| {
    let rows = shown.rows(breakdowns);
    // Over bars it lines up with their names, inset as far; over a table with its NAME column.
    let padding = Padding::new().bottom(SpacingSize::Xs);
    let padding = if table_shown {
      padding
    } else {
      padding.left(SpacingSize::Sm).right(SpacingSize::Sm)
    };
    let head = Row::new().width(Dimension::Pct(100.0)).padding(padding).child(
      Text::new(&ctx.t(shown.title_key()))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
    let body: Vec<Element> = if table_shown {
      vec![breakdown_table(ctx, dashboard, shown, (rows, spent))]
    } else {
      rows
        .iter()
        .take(ROWS)
        .enumerate()
        .map(|(index, entry)| bar_row(ctx, dashboard, (shown, index), (entry, rows), requery))
        .collect()
    };
    let idle = (shown == Breakdown::Projects && !breakdowns.idle_projects.is_empty()).then(|| {
      let names = breakdowns.idle_projects.join(&ctx.t("usage.listSeparator"));
      Text::new(&ctx.t_args("usage.breakdown.idle", [("projects", names.as_str())]))
        .id("usage-breakdown-idle")
        .variant(TypographyStyle::Caption)
        .padding(Padding::new().top(SpacingSize::Sm).left(SpacingSize::Sm))
    });
    Column::new()
      .id(shown.id())
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(head)
      .with_children(body)
      .with_children(idle)
  });
  let reviewed = breakdowns
    .profiles
    .iter()
    .any(|row| matches!(row.key, BreakdownKey::Reviewer(_)));
  let card = Card::new("usage-breakdown", ctx.t("usage.breakdown.title"))
    .note(ctx.t("usage.breakdown.note"))
    .trailing(switch);
  let card = if reviewed {
    card.subtitle(ctx.t("usage.breakdown.reviewerReadOnly"))
  } else {
    card
  };
  card.body([Row::new()
    .id("usage-breakdown-columns")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Section)
    .align_items(Alignment::Start)
    .with_children(columns)
    .into()])
}

/// A row's name: *Builder · Implement*, a model in mono, a pipeline, or that the pipeline is gone.
fn label(ctx: &Ctx, dashboard: &Dashboard, key: &BreakdownKey) -> (Arc<str>, bool) {
  match key {
    BreakdownKey::ProfileStep { profile, step } => (
      ctx.t_args(
        "usage.breakdown.profileStep",
        [("profile", &**profile), ("step", &**step)],
      ),
      false,
    ),
    BreakdownKey::Reviewer(model) | BreakdownKey::Model(model) => (model.clone(), true),
    BreakdownKey::Pipeline(Some(pipeline)) => (
      words::pipeline(ctx, pipeline, dashboard.pipeline_version(pipeline)),
      false,
    ),
    BreakdownKey::Pipeline(None) => (ctx.t("usage.breakdown.pipelineGone"), false),
    BreakdownKey::Project(_, name) => (name.clone(), false),
  }
}

/// A row's name as it reads: a model in mono 11; the reviewer's model, then *auto mode reviewer* muted.
fn name(ctx: &Ctx, dashboard: &Dashboard, key: &BreakdownKey, id: String) -> Element {
  let (text, mono) = label(ctx, dashboard, key);
  let text = Text::new(&text).id(id);
  let text = if mono {
    text
      .variant(ExtraTypography::MonoNote.role())
      .color(PaletteColor::TextPrimary)
  } else {
    text
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextPrimary)
  };
  let Some(_) = matches!(key, BreakdownKey::Reviewer(_)).then_some(()) else {
    return Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .child(fit(text))
      .into();
  };
  Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .clip()
    .child(text.nowrap().flex_shrink(0.0))
    .child(fit(
      Text::new(&ctx.t("usage.breakdown.reviewer")).variant(TypographyStyle::Caption),
    ))
    .into()
}

/// A bar row, 2 above the next (`Row · Builder · Implement` in `Where tokens go`).
const ROW_HEIGHT: f32 = 40.0;
/// A bar row's line of figures (`YAqCn`), which leaves its 6 high bar room in the row.
const FIGURES_LINE: f32 = 16.0;

fn bar_row(
  ctx: &Ctx,
  dashboard: &Dashboard,
  (shown, index): (Breakdown, usize),
  (entry, rows): (&BreakdownRow, &[BreakdownRow]),
  requery: &Requery,
) -> Element {
  let id = format!("{}-row-{index}", shown.id());
  let largest = rows.first().map_or(1, BreakdownRow::total).max(1);
  let spent = dashboard.report.summary.spent();
  let figures = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FIGURES_LINE))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(name(ctx, dashboard, &entry.key, format!("{id}-name")))
    .child(
      Text::new(&words::tokens(entry.total()))
        .id(format!("{id}-spent"))
        .variant(TypographyStyle::Mono)
        .nowrap(),
    )
    .child(
      Text::new(&words::share(ctx, percent(entry.total(), spent)))
        .id(format!("{id}-share"))
        .width(Dimension::Px(SHARE_WIDTH))
        .variant(TypographyStyle::Caption)
        .text_align(lurq::layout::text_style::TextAlign::Right)
        .nowrap(),
    );
  // Split by agent provider in series order and colours (`k9f4PW`); a reviewer's row by the providers it ran on.
  let split = dashboard.split(&entry.by_provider);
  let parts = ProviderSeries::ALL
    .into_iter()
    .map(|series| (series, split.of(series).counted()))
    .filter(|(_, spent)| *spent > 0)
    .map(|(series, spent)| {
      series_fill(
        Row::new()
          .flex(spent as f32)
          .height(Dimension::Px(BAR_HEIGHT))
          .rounded(UsageRadius::Bar.role()),
        series,
      )
    });
  let bar = Row::new()
    .width(Dimension::Pct(entry.total() as f32 / largest as f32 * 100.0))
    .height(Dimension::Px(BAR_HEIGHT))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(parts);
  let content = |frame: lurq::components::Button| {
    frame
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ROW_HEIGHT))
      .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
      .spacing(ExtraSpacing::Snug.role())
      .rounded(lurq::app::theme::RadiusSize::Md)
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Snug.role())
          .child(figures)
          .child(bar),
      )
  };
  match dashboard.with_row(&entry.key) {
    Some(query) => {
      let requery = requery.clone();
      content(pressable(move || requery.show(query.clone())))
        .id(id)
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .into()
    }
    // Auto mode's reviewer is no profile, and a pipeline that is gone cannot be chosen: the row reads.
    None => content(crate::ui::pressable::inert(lurq::components::Button::empty()))
      .id(id)
      .cursor(lurq::node::CursorIcon::Default)
      .into(),
  }
}

/// A column's rows with exact numbers: name, spent tokens, share.
fn breakdown_table(
  ctx: &Ctx,
  dashboard: &Dashboard,
  shown: Breakdown,
  (rows, spent): (&[BreakdownRow], u64),
) -> Element {
  let columns = [
    column(ctx.t("usage.table.name"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.spent"), Width::Px(SPENT_COLUMN), true),
    column(ctx.t("usage.table.share"), Width::Px(TABLE_SHARE_COLUMN), true),
  ];
  let id = format!("{}-table", shown.id());
  let rows = rows
    .iter()
    .enumerate()
    .map(|(index, entry)| {
      let row_id = format!("{id}-row-{index}");
      let cells = vec![
        Cell::Element(name(ctx, dashboard, &entry.key, format!("{row_id}-name"))),
        Cell::Text(words::exact(ctx, entry.total())),
        Cell::Text(words::share(ctx, percent(entry.total(), spent))),
      ];
      row(&row_id, &columns, Density::Data, cells).into()
    })
    .collect();
  table(ctx, &id, &columns, Density::Data, rows)
}
