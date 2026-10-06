//! *Summary* (`C2pTYF`): five figures in the Overview's metric style — spent tokens, runs, agent
//! time, cache share and failed runs — each with its change against the previous period of the same
//! length (an icon and words, points for the share; never colored good or bad) and what it counts.
//! While reading, every figure is a dash (`qQOKw`); without runs, counts are zero and shares a dash
//! (`AY6FM`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::Change;

use super::{format, state::Dashboard};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, IconSize, icon},
};

/// One figure.
struct Figure {
  id: &'static str,
  label: Arc<str>,
  value: Arc<str>,
  /// `None` while reading.
  change: Option<(Option<bool>, Arc<str>)>,
  detail: Arc<str>,
}

pub fn summary(ctx: &Ctx, dashboard: Option<&Dashboard>) -> Element {
  let figures = match dashboard {
    Some(dashboard) => figures(ctx, dashboard),
    None => reading(ctx),
  };
  Row::new()
    .id("usage-summary")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(figures.into_iter().map(figure))
    .into()
}

fn figures(ctx: &Ctx, dashboard: &Dashboard) -> Vec<Figure> {
  let (now, before) = (&dashboard.report.summary, &dashboard.report.previous);
  let previous = dashboard.range.previous();
  let change = |change: Option<Change>, points: bool| Some(format::change(ctx, change, previous, points));
  let count = |count: usize| u64::try_from(count).unwrap_or(u64::MAX);
  vec![
    Figure {
      id: "usage-summary-spent",
      label: ctx.t("usage.summary.spent"),
      value: format::tokens(now.spent()),
      change: change(Some(Change::between(now.spent(), before.spent())), false),
      detail: ctx.t("usage.summary.spentDetail"),
    },
    Figure {
      id: "usage-summary-runs",
      label: ctx.t("usage.summary.runs"),
      value: now.runs.to_string().into(),
      change: change(Some(Change::between(count(now.runs), count(before.runs))), false),
      detail: ctx.t_args(
        "usage.summary.runsDetail",
        [
          ("finished", now.finished.to_string().as_str()),
          ("failed", now.failed.to_string().as_str()),
        ],
      ),
    },
    Figure {
      id: "usage-summary-time",
      label: ctx.t("usage.summary.agentTime"),
      value: format::duration(ctx, now.agent_ms),
      change: change(Some(Change::between(now.agent_ms, before.agent_ms)), false),
      detail: ctx.t("usage.summary.agentTimeDetail"),
    },
    Figure {
      id: "usage-summary-cache",
      label: ctx.t("usage.summary.cacheShare"),
      value: format::share(ctx, now.cache_share()),
      change: change(Change::points(now.cache_share(), before.cache_share()), true),
      detail: ctx.t_args(
        "usage.summary.cacheDetail",
        [
          ("cached", &*format::tokens(now.usage.cached_input)),
          ("input", &*format::tokens(now.usage.input)),
        ],
      ),
    },
    Figure {
      id: "usage-summary-failed",
      label: ctx.t("usage.summary.failed"),
      value: now.failed.to_string().into(),
      change: change(Some(Change::between(count(now.failed), count(before.failed))), false),
      detail: ctx.t_args(
        "usage.summary.failedDetail",
        [
          ("failed", now.failed.to_string().as_str()),
          ("runs", now.runs.to_string().as_str()),
          ("tokens", &*format::tokens(now.failed_spent)),
        ],
      ),
    },
  ]
}

fn reading(ctx: &Ctx) -> Vec<Figure> {
  let unknown = ctx.t("usage.unknown");
  [
    (
      "usage-summary-spent",
      "usage.summary.spent",
      "usage.summary.spentDetail",
    ),
    ("usage-summary-runs", "usage.summary.runs", "usage.summary.reading"),
    (
      "usage-summary-time",
      "usage.summary.agentTime",
      "usage.summary.agentTimeDetail",
    ),
    (
      "usage-summary-cache",
      "usage.summary.cacheShare",
      "usage.summary.reading",
    ),
    ("usage-summary-failed", "usage.summary.failed", "usage.summary.reading"),
  ]
  .into_iter()
  .map(|(id, label, detail)| Figure {
    id,
    label: ctx.t(label),
    value: unknown.clone(),
    change: None,
    detail: ctx.t(detail),
  })
  .collect()
}

fn figure(figure: Figure) -> Element {
  let change = figure.change.map(|(direction, text)| {
    let glyph = direction.map(|more| if more { Icon::TrendingUp } else { Icon::TrendingDown });
    let row = Row::new()
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .with_children(glyph.map(|glyph| icon(glyph, IconSize::Small, PaletteColor::TextSecondary)))
      .child(Text::new(&text).variant(TypographyStyle::Caption).nowrap());
    Element::from(row)
  });
  Column::new()
    .id(figure.id)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .background(ExtraColor::Surface.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Lg)
    .child(Text::new(&figure.label).variant(TypographyStyle::Caption))
    .child(
      Text::new(&figure.value)
        .variant(ExtraTypography::Metric.role())
        .nowrap(),
    )
    .with_children(change)
    .child(
      Text::new(&figure.detail)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    )
    .into()
}
