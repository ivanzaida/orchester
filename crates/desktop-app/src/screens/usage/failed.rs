//! The *Failed runs* card of *Efficiency* (`Card · Failed runs` `Kc99a`): how many runs failed of all and
//! the tokens spent on them, each with its share; their share of the spent tokens as a two-part bar with
//! its legend; the failed run that spent the most; and *Show failed tasks →*, which opens the Overview's
//! *Failed* tab. As a table, the same figures with exact numbers.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{FailedRuns, percent};
use orchester_workspace::usage::Dashboard;

use super::{
  Requery, Views,
  card::{Card, push_down, swatch, text_link},
  efficiency::switch,
  table::{Cell, Density, Width, column, row, table},
  words,
};
use crate::{
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route},
  theme::{ExtraSpacing, UsageRadius, UsageText},
};

/// The share of spent tokens' bar (`Bar` `BBtL7`), and the table's columns.
const BAR_HEIGHT: f32 = 8.0;
const VALUE_COLUMN: f32 = 84.0;
const SHARE_COLUMN: f32 = 44.0;

pub fn card(ctx: &mut Ctx, dashboard: &Dashboard, views: &Signal<Views>, requery: &Requery) -> Element {
  let card = Card::new("usage-failed", ctx.t("usage.efficiency.failed"))
    .small()
    .subtitle(ctx.t("usage.efficiency.failedSubtitle"))
    .trailing(switch(ctx, "usage-failed-view", views, |views| &mut views.failed_table))
    .fill();
  let failed = &dashboard.report.efficiency.failed;
  if views.get().failed_table {
    return card.body([failed_table(ctx, failed)]);
  }
  let show = (failed.runs > 0).then(|| {
    let (navigation, route) = (
      Navigation::of(ctx),
      Route::Project(requery.project(), ProjectPage::Overview(OverviewTab::Failed)),
    );
    text_link("usage-show-failed", &ctx.t("usage.efficiency.showFailed"), move || {
      navigation.go(route.clone());
    })
  });
  card.body(
    [figures(ctx, failed), split(ctx, failed)]
      .into_iter()
      .chain(most(ctx, failed))
      .chain([push_down()])
      .chain(show),
  )
}

fn count(count: usize) -> u64 {
  u64::try_from(count).unwrap_or(u64::MAX)
}

/// *6 of 64* runs failed and the *4.1M* tokens spent on them, with their shares (`Figures` `iJXzF`).
fn figures(ctx: &Ctx, failed: &FailedRuns) -> Element {
  let figure = |id: &str, value: &str, caption: &str| {
    Column::new()
      .id(id.to_owned())
      .spacing(ExtraSpacing::Tight.role())
      .child(Text::new(value).variant(UsageText::Figure.role()).nowrap())
      .child(Text::new(caption).variant(TypographyStyle::Caption).nowrap())
  };
  let runs = ctx.t_args(
    "usage.efficiency.failedOf",
    [
      ("count", failed.runs.to_string().as_str()),
      ("runs", failed.of_runs.to_string().as_str()),
    ],
  );
  let runs_share = words::share(ctx, percent(count(failed.runs), count(failed.of_runs)));
  let tokens_share = words::share(ctx, percent(failed.spent, failed.of_spent));
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xl)
    .align_items(Alignment::Center)
    .child(figure(
      "usage-failed-runs",
      &runs,
      &ctx.t_args("usage.efficiency.runsFailed", [("share", &*runs_share)]),
    ))
    .child(figure(
      "usage-failed-tokens",
      &words::tokens(failed.spent),
      &ctx.t_args("usage.efficiency.spentOnThem", [("share", &*tokens_share)]),
    ))
    .into()
}

/// The failed runs' tokens against the other runs', as one bar with its legend (`Share of spent` `TdLmm`).
fn split(ctx: &Ctx, failed: &FailedRuns) -> Element {
  let other = failed.of_spent.saturating_sub(failed.spent);
  let (failed_color, other_color) = (PaletteColor::TextSecondary, PaletteColor::BorderStrong);
  let parts = [(failed.spent, failed_color.clone()), (other, other_color.clone())]
    .into_iter()
    .filter(|(tokens, _)| *tokens > 0)
    .map(|(tokens, color)| {
      Row::new()
        .flex(tokens as f32)
        .height(Dimension::Px(BAR_HEIGHT))
        .background(color)
        .rounded(UsageRadius::Bar.role())
    });
  let entry = |color: PaletteColor, key: &str, tokens: u64| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(swatch(color))
      .child(
        Text::new(&ctx.t_args(key, [("tokens", &*words::tokens(tokens))]))
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      )
  };
  Column::new()
    .id("usage-failed-split")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(BAR_HEIGHT))
        .spacing(ExtraSpacing::Tight.role())
        .with_children(parts),
    )
    .child(
      Row::new()
        .spacing(SpacingSize::Lg)
        .align_items(Alignment::Center)
        .child(entry(failed_color, "usage.efficiency.failedPart", failed.spent))
        .child(entry(other_color, "usage.efficiency.otherPart", other)),
    )
    .into()
}

/// *Most: KON-11 · Run 2 · 2.4M*, the failed run that spent the most.
fn most(ctx: &Ctx, failed: &FailedRuns) -> Option<Element> {
  let largest = failed.largest.as_ref()?;
  let text = ctx.t_args(
    "usage.efficiency.most",
    [
      ("run", &*words::run_label(ctx, &largest.key, largest.number.get())),
      ("tokens", &*words::tokens(largest.usage.counted())),
    ],
  );
  Some(
    Text::new(&text)
      .id("usage-failed-most")
      .variant(TypographyStyle::Description)
      .nowrap()
      .into(),
  )
}

/// The same figures with exact numbers.
fn failed_table(ctx: &Ctx, failed: &FailedRuns) -> Element {
  let columns = [
    column(ctx.t("usage.table.name"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.value"), Width::Px(VALUE_COLUMN), true),
    column(ctx.t("usage.table.share"), Width::Px(SHARE_COLUMN), true),
  ];
  let rows = vec![
    row(
      "usage-failed-table-runs",
      &columns,
      Density::Data,
      vec![
        Cell::Text(ctx.t("usage.efficiency.failed")),
        Cell::Text(ctx.t_args(
          "usage.efficiency.failedOf",
          [
            ("count", failed.runs.to_string().as_str()),
            ("runs", failed.of_runs.to_string().as_str()),
          ],
        )),
        Cell::Text(words::share(ctx, percent(count(failed.runs), count(failed.of_runs)))),
      ],
    )
    .into(),
    row(
      "usage-failed-table-tokens",
      &columns,
      Density::Data,
      vec![
        Cell::Text(ctx.t("usage.efficiency.failedTokens")),
        Cell::Text(words::exact(ctx, failed.spent)),
        Cell::Text(words::share(ctx, percent(failed.spent, failed.of_spent))),
      ],
    )
    .into(),
  ];
  table("usage-failed-table", &columns, Density::Data, rows)
}
