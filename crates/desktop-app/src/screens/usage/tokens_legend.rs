//! *Tokens over time*'s legend (`Legend` `CuobA`, two rows in `EA7QF`): each series with usage in the range,
//! its swatch, its label and its total for the measure, in series order, wrapping onto a second row; *Earlier
//! runs* last with an ⓘ that says what it holds (in the look of `Legend item/no account` `XpTjp`, `m6cFFt`); and
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
  node::{Element, HitTestBehavior, dimension::Dimension, padding::Padding},
};
use orchester_core::usage::{Grouping, Measure, ProviderSeries};
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
/// An item's line, with the 6 under it a row of 22 (`EA7QF`).
pub const ITEM_HEIGHT: f32 = 22.0;

/// The legend over the chart.
pub fn legend(ctx: &mut Ctx, dashboard: &Dashboard, measure: Measure) -> Element {
  let items: Vec<Element> = dashboard
    .shown_series()
    .into_iter()
    .map(|shown| {
      let (series, total) = (shown.series, measure.of(shown.usage));
      let name = series_text(ctx, &shown.label);
      let id = format!("usage-tokens-legend-{}", series.index());
      let info = (series == ProviderSeries::Earlier).then(|| ctx.mount_keyed::<EarlierInfo>("usage-earlier-info", ()));
      Row::new()
        .id(id)
        .height(Dimension::Px(ITEM_HEIGHT))
        .padding(Padding::new().right(SpacingSize::Lg).bottom(ExtraSpacing::Snug.role()))
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
    .spacing(ExtraSpacing::Fine.role())
    .child(
      // Each item keeps 16 after it and 6 under it: one gap would space the rows as wide as the items.
      Row::new()
        .width(Dimension::Pct(100.0))
        .wrap()
        .align_items(Alignment::Start)
        .with_children(items),
    )
    .child(
      Text::new(&ctx.t(caption_key(measure, dashboard.grouping)))
        .id("usage-tokens-caption")
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .into()
}

/// What the bars count, per day or per hour: its key.
pub const fn caption_key(measure: Measure, grouping: Grouping) -> &'static str {
  match (measure, grouping) {
    (Measure::Spent, Grouping::Day) => "usage.tokens.perDay.spent",
    (Measure::Cached, Grouping::Day) => "usage.tokens.perDay.cached",
    (Measure::Output, Grouping::Day) => "usage.tokens.perDay.output",
    (Measure::Spent, Grouping::Hour) => "usage.tokens.perHour.spent",
    (Measure::Cached, Grouping::Hour) => "usage.tokens.perHour.cached",
    (Measure::Output, Grouping::Hour) => "usage.tokens.perHour.output",
  }
}

/// The ⓘ's room, no taller than the legend's line.
const ICON_ROOM: f32 = 16.0;

/// The ⓘ after *Earlier runs*, which says what it holds while the pointer rests on it (`m6cFFt`).
pub struct EarlierInfo {
  shown: Signal<bool>,
  anchor: ElementRef,
}

impl Component for EarlierInfo {
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
      .id("usage-earlier-info")
      .ref_element(self.anchor.clone())
      .height(Dimension::Px(ICON_ROOM))
      .align_items(Alignment::Center)
      .describe("a11y_name", ctx.t("usage.providers.earlierInfo"))
      .on_mouse_enter(move || show.set(true))
      .on_mouse_leave(move || hide.set(false))
      .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted));
    if !self.shown.get() {
      return anchor;
    }
    let panel = tooltip::info_tip(
      "usage-earlier-tooltip",
      (
        series_swatch(ProviderSeries::Earlier),
        &ctx.t("usage.providers.earlier"),
      ),
      &ctx.t("usage.providers.earlierInfo"),
    );
    anchor.child(
      Popup::new(self.anchor.clone(), panel)
        .open_when(true)
        .placement(Placement::Bottom)
        .offset(0.0, ExtraSpacing::Snug.value())
        .hit_test(HitTestBehavior::None)
        .dismiss_on_outside_click(false)
        .dismiss_on_escape(false),
    )
  }
}
