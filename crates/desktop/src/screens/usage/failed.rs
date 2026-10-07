//! The *Failed runs* card of *Efficiency* (`C2pTYF`): how many runs failed of all and the tokens
//! spent on them, each with its share; their share of spent tokens as a two-part bar labelled at its
//! ends; the failed run that spent the most and *Show failed tasks →*, which opens the Overview's
//! *Failed* filter.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{FailedRuns, percent};

use super::{
  card::{Card, push_down, text_link},
  efficiency::switch,
  format,
  load::Scope,
  state::{Dashboard, Views},
  table::{Cell, Width, column, data_row, table},
};
use crate::{
  navigation::route::Page,
  screens::project::ProjectNav,
  tasks::overview::OverviewTab,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, chart::BREAKDOWN_BAR_HEIGHT},
  ui::{
    icon::{Icon, IconSize, icon},
    link::Press,
  },
};

pub fn failed(ctx: &mut Ctx, dashboard: &Dashboard, views: &Signal<Views>) -> Element {
  let card = Card::new("usage-failed", ctx.t("usage.efficiency.failed"))
    .subtitle(ctx.t("usage.efficiency.failedSubtitle"))
    .trailing(switch(ctx, ("usage-failed-view", Icon::ChartColumn), views, |views| {
      &mut views.failed_table
    }))
    .fill();
  let failed = &dashboard.report.efficiency.failed;
  if views.get().failed_table {
    return card.body([failed_table(ctx, failed)]);
  }
  let nav = ctx.use_context::<ProjectNav>();
  card.body([
    figures(ctx, failed),
    split(ctx, failed),
    push_down(),
    footer(ctx, dashboard, nav),
  ])
}

fn count(count: usize) -> u64 {
  u64::try_from(count).unwrap_or(u64::MAX)
}

/// *6 of 64* runs failed and the *3.5M* tokens spent on them, with their shares.
fn figures(ctx: &Ctx, failed: &FailedRuns) -> Element {
  let figure = |value: &str, caption: &str| {
    Element::from(
      Column::new()
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(value).variant(ExtraTypography::Metric.role()).nowrap())
        .child(Text::new(caption).variant(TypographyStyle::Caption).nowrap()),
    )
  };
  let runs = ctx.t_args(
    "usage.efficiency.failedOf",
    [
      ("count", failed.runs.to_string().as_str()),
      ("runs", failed.of_runs.to_string().as_str()),
    ],
  );
  let runs_share = format::share(ctx, percent(count(failed.runs), count(failed.of_runs)));
  let tokens_share = format::share(ctx, percent(failed.spent, failed.of_spent));
  Row::new()
    .spacing(SpacingSize::Lg)
    .child(figure(
      &runs,
      &ctx.t_args("usage.efficiency.runsFailed", [("share", &*runs_share)]),
    ))
    .child(figure(
      &format::tokens(failed.spent),
      &ctx.t_args("usage.efficiency.spentOnThem", [("share", &*tokens_share)]),
    ))
    .into()
}

/// The failed runs' tokens against the other runs', as one bar labelled at both ends.
fn split(ctx: &Ctx, failed: &FailedRuns) -> Element {
  let other = failed.of_spent.saturating_sub(failed.spent);
  let part = |tokens: u64, color: PaletteColor| {
    Element::from(
      Row::new()
        .flex(tokens as f32)
        .height(Dimension::Px(BREAKDOWN_BAR_HEIGHT))
        .background(color),
    )
  };
  let parts = [
    (failed.spent, PaletteColor::TextSecondary),
    (other, PaletteColor::Border),
  ]
  .into_iter()
  .filter(|(tokens, _)| *tokens > 0)
  .map(|(tokens, color)| part(tokens, color));
  let label = |key: &str, tokens: u64| {
    Text::new(&ctx.t_args(key, [("tokens", &*format::tokens(tokens))]))
      .variant(TypographyStyle::Caption)
      .nowrap()
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(BREAKDOWN_BAR_HEIGHT))
        .spacing(ExtraSpacing::Hair.role())
        .rounded(ExtraRadius::Mark.role())
        .background(PaletteColor::Border)
        .with_children(parts),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .child(label("usage.efficiency.failedPart", failed.spent))
        .child(Spacer::new().flex(1.0))
        .child(label("usage.efficiency.otherPart", other)),
    )
    .into()
}

/// The failed run that spent the most, and *Show failed tasks →* on a project's page.
fn footer(ctx: &Ctx, dashboard: &Dashboard, nav: Option<ProjectNav>) -> Element {
  let failed = &dashboard.report.efficiency.failed;
  let most = failed.largest.as_ref().map(|largest| {
    let text = if dashboard.all_projects() {
      ctx.t_args(
        "usage.efficiency.mostIn",
        [
          ("project", &*largest.project_name),
          ("task", &*largest.key),
          ("run", largest.number.get().to_string().as_str()),
          ("tokens", &*format::tokens(largest.usage.counted())),
        ],
      )
    } else {
      ctx.t_args(
        "usage.efficiency.most",
        [
          ("task", &*largest.key),
          ("run", largest.number.get().to_string().as_str()),
          ("tokens", &*format::tokens(largest.usage.counted())),
        ],
      )
    };
    Element::from(
      Row::new()
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleX, IconSize::Small, PaletteColor::TextMuted))
        .child(Text::new(&text).variant(TypographyStyle::Caption).nowrap()),
    )
  });
  let show = match (&dashboard.scope, nav) {
    (Scope::Project(_), Some(nav)) if failed.runs > 0 => Some(text_link(
      "usage-show-failed",
      &ctx.t("usage.efficiency.showFailed"),
      Press::Link(nav.link(Page::Overview(OverviewTab::Failed))),
    )),
    _ => None,
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(most)
    .child(Spacer::new().flex(1.0))
    .with_children(show)
    .into()
}

/// The same figures with exact numbers.
fn failed_table(ctx: &Ctx, failed: &FailedRuns) -> Element {
  let columns = [
    column(ctx.t("usage.table.name"), Width::Flex(1.4), false),
    column(ctx.t("usage.table.value"), Width::Flex(1.0), true),
    column(ctx.t("usage.table.share"), Width::Flex(0.6), true),
  ];
  let rows = vec![
    data_row(
      &columns,
      vec![
        Cell::Text(ctx.t("usage.efficiency.failed")),
        Cell::Text(ctx.t_args(
          "usage.efficiency.failedOf",
          [
            ("count", failed.runs.to_string().as_str()),
            ("runs", failed.of_runs.to_string().as_str()),
          ],
        )),
        Cell::Text(format::share(ctx, percent(count(failed.runs), count(failed.of_runs)))),
      ],
    ),
    data_row(
      &columns,
      vec![
        Cell::Text(ctx.t("usage.efficiency.failedTokens")),
        Cell::Text(format::exact(ctx, failed.spent)),
        Cell::Text(format::share(ctx, percent(failed.spent, failed.of_spent))),
      ],
    ),
  ];
  table("usage-failed-table", &columns, rows)
}
