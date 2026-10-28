//! The *Failed runs* card of *Efficiency* (`Card · Failed runs` `Kc99a`): how many runs failed of all and
//! the tokens spent on them, each with its share; their share of the spent tokens as a two-part bar with
//! its legend; the failed run that spent the most; and *Show failed tasks →*, which opens Tasks › Active of the
//! project the failed runs belong to, where failed tasks are listed under *Stopped* (on every project's page, while
//! they belong to one). Its figures are exact already, so it has no table view.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{FailedRuns, percent};
use orchester_workspace::usage::Dashboard;

use super::{
  card::{Card, push_down, swatch, text_link},
  words,
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList, TaskListTab},
  theme::{ExtraSpacing, UsageRadius, UsageText},
};

/// The share of spent tokens' bar (`Bar` `BBtL7`).
const BAR_HEIGHT: f32 = 8.0;

pub fn card(ctx: &mut Ctx, dashboard: &Dashboard) -> Element {
  let card = Card::new("usage-failed", ctx.t("usage.efficiency.failed"))
    .small()
    .subtitle(ctx.t("usage.efficiency.failedSubtitle"))
    .fill();
  let failed = &dashboard.report.efficiency.failed;
  let show = dashboard.failed_project().map(|project| {
    let route = Route::Project(project, ProjectPage::Tasks(TaskList::new(TaskListTab::Active)));
    text_link(
      "usage-show-failed",
      &ctx.t("usage.efficiency.showFailed"),
      Navigation::of(ctx).to(route),
    )
  });
  card.body(
    [figures(ctx, failed), split(ctx, failed)]
      .into_iter()
      .chain(most(ctx, failed, dashboard.project.is_none()))
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

/// *Most: KON-11 · Run 2 · 2.4M*, the failed run that spent the most; on every project's page with the run's
/// project first.
fn most(ctx: &Ctx, failed: &FailedRuns, everywhere: bool) -> Option<Element> {
  let largest = failed.largest.as_ref()?;
  let run = words::run_label(ctx, &largest.key, largest.number.get());
  let run = if everywhere {
    ctx.t_args(
      "usage.efficiency.projectRun",
      [("project", &*largest.project_name), ("run", &*run)],
    )
  } else {
    run
  };
  let text = ctx.t_args(
    "usage.efficiency.most",
    [("run", &*run), ("tokens", &*words::tokens(largest.usage.counted()))],
  );
  Some(
    Text::new(&text)
      .id("usage-failed-most")
      .variant(TypographyStyle::Description)
      .nowrap()
      .into(),
  )
}
