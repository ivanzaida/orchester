//! *Tokens over time*'s legend (`Legend` `CuobA`, two rows in `EA7QF`): each series with usage in the range,
//! its swatch, its label and its total for the measure, in series order, wrapping onto a second row; *No
//! account* last with an ⓘ that says what has no account (`Legend item/no account` `XpTjp`, `m6cFFt`); and
//! under them what the bars count.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension},
};
use orchester_core::usage::{AccountSeries, Grouping, Measure};
use orchester_workspace::usage::Dashboard;

use super::{
  series::{series_swatch, series_text},
  tooltip, words,
};
use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

/// The legend's room over the chart, as tall as *Plan limits*' two rows of windows beside it, so the two
/// plots line up (`Legend` `CuobA`).
pub const LEGEND_HEIGHT: f32 = 72.0;

/// The legend over the chart.
pub fn legend(ctx: &mut Ctx, dashboard: &Dashboard, measure: Measure) -> Element {
  let totals = series_totals(dashboard, measure);
  let items: Vec<Element> = totals
    .into_iter()
    .map(|(series, total)| {
      let name = series_text(ctx, &dashboard.series_label(series));
      let id = format!("usage-tokens-legend-{}", series.index());
      let info =
        (series == AccountSeries::NoAccount).then(|| ctx.mount_keyed::<NoAccountInfo>("usage-no-account-info", ()));
      Row::new()
        .id(id)
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(series_swatch(series))
        .child(Text::new(&name).variant(TypographyStyle::Description).nowrap())
        .child(Text::new(&words::tokens(total)).variant(TypographyStyle::Mono).nowrap())
        .with_children(info)
        .into()
    })
    .collect();
  Column::new()
    .id("usage-tokens-legend")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LEGEND_HEIGHT))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .wrap()
        .spacing(SpacingSize::Lg)
        .align_items(Alignment::Center)
        .with_children(items),
    )
    .child(
      Text::new(&caption(ctx, dashboard, measure))
        .id("usage-tokens-caption")
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .into()
}

/// Each series with usage in the range and its total for `measure`, in series order.
fn series_totals(dashboard: &Dashboard, measure: Measure) -> Vec<(AccountSeries, u64)> {
  let mut totals = [0_u64; orchester_core::usage::SERIES];
  for bar in &dashboard.report.series {
    let split = dashboard.split(&bar.by_account);
    for series in AccountSeries::ALL {
      totals[series.index()] += measure.of(split.of(series));
    }
  }
  dashboard
    .shown_series()
    .into_iter()
    .map(|series| (series, totals[series.index()]))
    .collect()
}

/// What the bars count, per day or per hour.
fn caption(ctx: &Ctx, dashboard: &Dashboard, measure: Measure) -> std::sync::Arc<str> {
  ctx.t(match (measure, dashboard.grouping) {
    (Measure::Spent, Grouping::Day) => "usage.tokens.perDay.spent",
    (Measure::Cached, Grouping::Day) => "usage.tokens.perDay.cached",
    (Measure::Output, Grouping::Day) => "usage.tokens.perDay.output",
    (Measure::Spent, Grouping::Hour) => "usage.tokens.perHour.spent",
    (Measure::Cached, Grouping::Hour) => "usage.tokens.perHour.cached",
    (Measure::Output, Grouping::Hour) => "usage.tokens.perHour.output",
  })
}

/// The ⓘ after *No account*, which says what has no account while the pointer rests on it (`m6cFFt`).
pub struct NoAccountInfo {
  shown: Signal<bool>,
  anchor: ElementRef,
}

impl Component for NoAccountInfo {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      shown: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let (show, hide) = (self.shown.clone(), self.shown.clone());
    let anchor = Row::new()
      .id("usage-no-account-info")
      .ref_element(self.anchor.clone())
      .describe("a11y_name", ctx.t("usage.accounts.noneInfo"))
      .on_mouse_enter(move || show.set(true))
      .on_mouse_leave(move || hide.set(false))
      .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted));
    if !self.shown.get() {
      return anchor;
    }
    let panel = tooltip::info_tip(
      "usage-no-account-tooltip",
      (series_swatch(AccountSeries::NoAccount), &ctx.t("usage.accounts.none")),
      &ctx.t("usage.accounts.noneInfo"),
    );
    anchor.child(
      Popup::new(self.anchor.clone(), panel)
        .open_when(true)
        .placement(Placement::BottomStart)
        .offset(0.0, ExtraSpacing::Snug.value())
        .hit_test(HitTestBehavior::None)
        .dismiss_on_outside_click(false)
        .dismiss_on_escape(false),
    )
  }
}
