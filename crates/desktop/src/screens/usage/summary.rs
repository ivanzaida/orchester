//! *Summary* (`C2pTYF`): five plain figures in the Overview's metric style over a hairline — spent
//! tokens, runs, agent time, cache share and the share of failed runs — each with its change against
//! the previous period of the same length (an icon and words: counts for runs, points for shares;
//! never colored good or bad) and what it counts. While reading, every figure is a dash (`qQOKw`);
//! without runs, counts are zero, shares a dash and the change line gives the previous period's
//! value (`AY6FM`); when the filters match nothing, the figures say so (`YxAed`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::usage::{Change, Summary, local_date};

use super::{
  format::{self, ChangeUnit},
  state::Dashboard,
};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, IconSize, icon},
};

/// One figure.
struct Figure {
  id: &'static str,
  label: Arc<str>,
  value: Arc<str>,
  /// The icon and the words under the value; `None` while reading.
  change: Option<(Option<Icon>, Arc<str>)>,
  detail: Option<Arc<str>>,
}

/// The five figures, in order: what each is called and its id.
const FIGURES: [(&str, &str); 5] = [
  ("usage-summary-spent", "usage.summary.spent"),
  ("usage-summary-runs", "usage.summary.runs"),
  ("usage-summary-time", "usage.summary.agentTime"),
  ("usage-summary-cache", "usage.summary.cacheShare"),
  ("usage-summary-failed", "usage.summary.failed"),
];

pub fn summary(ctx: &Ctx, dashboard: Option<&Dashboard>) -> Element {
  let figures = match dashboard {
    Some(dashboard) if dashboard.filtered_out => unmatched(ctx),
    Some(dashboard) if dashboard.no_runs() => idle(ctx, dashboard),
    Some(dashboard) => figures(ctx, dashboard),
    None => reading(ctx),
  };
  Row::new()
    .id("usage-summary")
    .width(Dimension::Pct(100.0))
    .padding_bottom(SpacingSize::Lg)
    .spacing(SpacingSize::Lg)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(figures.into_iter().map(figure))
    .into()
}

/// The five values of `summary`: counts as they are, shares as a dash when unknown.
fn values(ctx: &Ctx, summary: &Summary) -> [Arc<str>; 5] {
  [
    format::tokens(summary.spent()),
    summary.runs.to_string().into(),
    format::duration(ctx, summary.agent_ms),
    format::share(ctx, summary.cache_share()),
    format::share(ctx, summary.failed_share()),
  ]
}

fn figures(ctx: &Ctx, dashboard: &Dashboard) -> Vec<Figure> {
  let (now, before) = (&dashboard.report.summary, &dashboard.report.previous);
  let previous = format::previous_name(ctx, dashboard.range.previous(), local_date(dashboard.now));
  let change = |change: Option<Change>, unit: ChangeUnit| {
    let (direction, text) = format::change(ctx, change, &previous, unit);
    let glyph = direction.map(|more| if more { Icon::TrendingUp } else { Icon::TrendingDown });
    Some((glyph, text))
  };
  let count = |count: usize| u64::try_from(count).unwrap_or(u64::MAX);
  let changes = [
    change(Some(Change::between(now.spent(), before.spent())), ChangeUnit::Percent),
    change(
      Some(Change::count(count(now.runs), count(before.runs))),
      ChangeUnit::Count,
    ),
    change(
      Some(Change::between(now.agent_ms, before.agent_ms)),
      ChangeUnit::Percent,
    ),
    change(
      Change::points(now.cache_share(), before.cache_share()),
      ChangeUnit::Points,
    ),
    change(
      Change::points(now.failed_share(), before.failed_share()),
      ChangeUnit::Points,
    ),
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
        ("cached", &*format::tokens(now.usage.cached_input)),
        ("input", &*format::tokens(now.usage.input)),
      ],
    ),
    ctx.t_args(
      "usage.summary.failedDetail",
      [
        ("failed", now.failed.to_string().as_str()),
        ("runs", now.runs.to_string().as_str()),
        ("tokens", &*format::tokens(now.failed_spent)),
      ],
    ),
  ];
  assemble(ctx, values(ctx, now), changes, details.map(Some))
}

/// No runs in the period (`AY6FM`): zeros and dashes, and the previous period's values.
fn idle(ctx: &Ctx, dashboard: &Dashboard) -> Vec<Figure> {
  let previous = format::previous_name(ctx, dashboard.range.previous(), local_date(dashboard.now));
  let changes = values(ctx, &dashboard.report.previous).map(|value| {
    let text = ctx.t_args("usage.summary.before", [("range", &*previous), ("value", &*value)]);
    Some((Some(Icon::History), text))
  });
  let keys = if dashboard.is_today() {
    [
      "usage.summary.idleToday.runs",
      "usage.summary.idleToday.runs",
      "usage.summary.idleToday.agents",
      "usage.summary.idleToday.input",
      "usage.summary.idleToday.runs",
    ]
  } else {
    [
      "usage.summary.idle.runs",
      "usage.summary.idle.runs",
      "usage.summary.idle.agents",
      "usage.summary.idle.input",
      "usage.summary.idle.runs",
    ]
  };
  let details = keys.map(|key| Some(ctx.t(key)));
  assemble(ctx, values(ctx, &Summary::default()), changes, details)
}

/// The filters match nothing (`YxAed`).
fn unmatched(ctx: &Ctx) -> Vec<Figure> {
  let details = [
    "usage.summary.unmatched",
    "usage.summary.unmatched",
    "usage.summary.unmatched",
    "usage.summary.noInput",
    "usage.summary.unmatched",
  ]
  .map(|key| Some(ctx.t(key)));
  assemble(
    ctx,
    values(ctx, &Summary::default()),
    [None, None, None, None, None],
    details,
  )
}

fn reading(ctx: &Ctx) -> Vec<Figure> {
  let unknown = ctx.t("usage.unknown");
  let values = [(); 5].map(|()| unknown.clone());
  assemble(
    ctx,
    values,
    [None, None, None, None, None],
    [None, None, None, None, None],
  )
}

/// The figures from their parts, in [`FIGURES`] order.
fn assemble(
  ctx: &Ctx,
  values: [Arc<str>; 5],
  changes: [Option<(Option<Icon>, Arc<str>)>; 5],
  details: [Option<Arc<str>>; 5],
) -> Vec<Figure> {
  FIGURES
    .into_iter()
    .zip(values)
    .zip(changes)
    .zip(details)
    .map(|((((id, label), value), change), detail)| Figure {
      id,
      label: ctx.t(label),
      value,
      change,
      detail,
    })
    .collect()
}

fn figure(figure: Figure) -> Element {
  let change = figure.change.map(|(glyph, text)| {
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .with_children(glyph.map(|glyph| icon(glyph, IconSize::Small, PaletteColor::TextSecondary)))
      .child(Text::new(&text).variant(TypographyStyle::Caption).flex_shrink(1.0));
    Element::from(row)
  });
  let detail = figure.detail.map(|detail| {
    Element::from(
      Text::new(&detail)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    )
  });
  Column::new()
    .id(figure.id)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(Text::new(&figure.label).variant(TypographyStyle::Description))
    .child(
      Text::new(&figure.value)
        .variant(ExtraTypography::Metric.role())
        .nowrap(),
    )
    .with_children(change)
    .with_children(detail)
    .into()
}
