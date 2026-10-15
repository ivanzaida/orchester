//! *Summary* (`Summary` `CY4ZI`, `Summary figure` `R22iW`): spent tokens, runs, agent time, cache share
//! and the share of failed runs, each on `surface_raised` with its change against the period before (an
//! icon and words: runs in runs, shares in points; never good or bad) and what it counts. While reading,
//! every figure is a dash and says nothing more (`sAQ7z`); without runs, counts are zero, shares a dash and
//! the change line gives the period before's value (`EKcoY`); when the filters match nothing, the figures
//! say so (`YxAed`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{Change, Summary};
use orchester_workspace::usage::{Dashboard, DashboardState};

use super::words::{self, ChangeUnit};
use crate::{
  theme::{ExtraTypography, IconSize},
  ui::{
    card::{CardSurface, card},
    icon::{Icon, icon},
  },
};

/// The five figures, in order: their ids and names.
const FIGURES: [(&str, &str); 5] = [
  ("usage-summary-spent", "usage.summary.spent"),
  ("usage-summary-runs", "usage.summary.runs"),
  ("usage-summary-time", "usage.summary.agentTime"),
  ("usage-summary-cache", "usage.summary.cacheShare"),
  ("usage-summary-failed", "usage.summary.failedRuns"),
];

/// What one figure says under its value: the change's icon and words.
type ChangeLine = Option<(Option<Icon>, Arc<str>)>;

pub fn summary(ctx: &Ctx, dashboard: Option<&Dashboard>) -> Element {
  let (values, changes, details) = match dashboard {
    Some(dashboard) => match dashboard.state {
      DashboardState::Charts => figures(ctx, dashboard),
      DashboardState::NoRuns => idle(ctx, dashboard),
      DashboardState::FilteredOut { .. } => unmatched(ctx),
    },
    None => reading(ctx),
  };
  let cards = FIGURES
    .into_iter()
    .zip(values)
    .zip(changes)
    .zip(details)
    .map(|((((id, label), value), change), detail)| figure(ctx, (id, label), &value, change, detail));
  Row::new()
    .id("usage-summary")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .with_children(cards)
    .into()
}

type Figures = ([Arc<str>; 5], [ChangeLine; 5], [Option<Arc<str>>; 5]);

/// The five values of `summary`: counts as they are, shares as a dash when unknown.
fn values(ctx: &Ctx, summary: &Summary) -> [Arc<str>; 5] {
  [
    words::tokens(summary.spent()),
    summary.runs.to_string().into(),
    words::duration(ctx, summary.agent_ms),
    words::share(ctx, summary.cache_share()),
    words::share(ctx, summary.failed_share()),
  ]
}

fn figures(ctx: &Ctx, dashboard: &Dashboard) -> Figures {
  let now = &dashboard.report.summary;
  let previous = words::previous_name(ctx, dashboard.previous(), dashboard.today());
  let line = |change: Option<Change>, unit: ChangeUnit| {
    let (up, text) = words::change(ctx, change, &previous, unit);
    Some((
      up.map(|up| if up { Icon::TrendingUp } else { Icon::TrendingDown }),
      text,
    ))
  };
  let changes = dashboard.changes();
  let lines = [
    line(Some(changes.spent), ChangeUnit::Percent),
    line(Some(changes.runs), ChangeUnit::Count),
    line(Some(changes.agent_time), ChangeUnit::Percent),
    line(changes.cache_share, ChangeUnit::Points),
    line(changes.failed_share, ChangeUnit::Points),
  ];
  let details = [
    ctx.t("usage.summary.spentDetail"),
    ctx.t_args(
      "usage.summary.runsDetail",
      [
        ("finished", now.finished.to_string().as_str()),
        ("failed", now.failed.to_string().as_str()),
      ],
    ),
    ctx.t("usage.summary.agentTimeDetail"),
    ctx.t_args(
      "usage.summary.cacheDetail",
      [
        ("cached", &*words::tokens(now.usage.cached_input)),
        ("input", &*words::tokens(now.usage.input)),
      ],
    ),
    ctx.t_args(
      "usage.summary.failedDetail",
      [
        ("failed", now.failed.to_string().as_str()),
        ("runs", now.runs.to_string().as_str()),
        ("tokens", &*words::tokens(now.failed_spent)),
      ],
    ),
  ];
  (values(ctx, now), lines, details.map(Some))
}

/// No runs in the period (`EKcoY`): zeros and dashes, and the period before's values.
fn idle(ctx: &Ctx, dashboard: &Dashboard) -> Figures {
  let previous = words::previous_name(ctx, dashboard.previous(), dashboard.today());
  let lines = values(ctx, &dashboard.report.previous).map(|value| {
    let text = ctx.t_args("usage.summary.before", [("range", &*previous), ("value", &*value)]);
    Some((Some(Icon::History), text))
  });
  let keys = if dashboard.is_today() {
    [
      "usage.summary.idle.today.runs",
      "usage.summary.idle.today.runs",
      "usage.summary.idle.today.agents",
      "usage.summary.idle.today.input",
      "usage.summary.idle.today.runs",
    ]
  } else {
    [
      "usage.summary.idle.period.runs",
      "usage.summary.idle.period.runs",
      "usage.summary.idle.period.agents",
      "usage.summary.idle.period.input",
      "usage.summary.idle.period.runs",
    ]
  };
  let details = keys.map(|key| Some(ctx.t(key)));
  (values(ctx, &Summary::default()), lines, details)
}

/// The filters match nothing (`YxAed`).
fn unmatched(ctx: &Ctx) -> Figures {
  let details = [
    "usage.summary.unmatched",
    "usage.summary.unmatched",
    "usage.summary.unmatched",
    "usage.summary.noInput",
    "usage.summary.unmatched",
  ]
  .map(|key| Some(ctx.t(key)));
  (
    values(ctx, &Summary::default()),
    [None, None, None, None, None],
    details,
  )
}

fn reading(ctx: &Ctx) -> Figures {
  let unknown = ctx.t("usage.unknown");
  (
    [(); 5].map(|()| unknown.clone()),
    [None, None, None, None, None],
    [None, None, None, None, None],
  )
}

fn figure(ctx: &Ctx, (id, label): (&str, &str), value: &str, change: ChangeLine, detail: Option<Arc<str>>) -> Element {
  let change = change.map(|(glyph, text)| {
    Row::new()
      .id(format!("{id}-change"))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .with_children(glyph.map(|glyph| icon(glyph, IconSize::Sm, PaletteColor::TextMuted)))
      .child(
        Text::new(&text)
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextSecondary)
          .nowrap()
          .flex_shrink(1.0),
      )
  });
  let detail = detail.map(|detail| {
    Text::new(&detail)
      .id(format!("{id}-detail"))
      .variant(TypographyStyle::Caption)
      .nowrap()
  });
  card(id, CardSurface::Raised)
    .width(Dimension::Px(0.0))
    .flex(1.0)
    .padding(SpacingSize::Lg)
    .spacing(SpacingSize::Xs)
    .child(Text::new(&ctx.t(label)).variant(TypographyStyle::FieldLabel).nowrap())
    .child(
      Text::new(value)
        .id(format!("{id}-value"))
        .variant(ExtraTypography::Display.role())
        .nowrap(),
    )
    .with_children(change)
    .with_children(detail)
    .into()
}
