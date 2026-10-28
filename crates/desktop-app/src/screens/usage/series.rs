//! How a series looks (`Chart colors` `FDC8B`, `Series hatch` `LEQ1w`): agent providers 1–3 in `chart_1`–`chart_3`,
//! Other providers in `chart_other`, Earlier runs hatched — `chart_none` stripes and outline over `chart_hatch` — and
//! the words of its label (*Claude · DeepSeek* as the Connections page names it, *Other providers · 2*, *Earlier
//! runs*). The design draws accounts in these places (`P0pTa`); providers take their look (decided 2026-10-28). *Plan
//! limits* draws accounts, whose limits they are, in the same palette, and names them (*Codex · Client*).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor},
  },
  components::Row,
  node::{Element, Gradient, GradientStop, border::Border, dimension::Dimension},
};
use orchester_core::{
  account::AccountCli,
  usage::{AccountSeries, ProviderSeries},
};
use orchester_workspace::usage::{AccountLabel, ProviderLabel, SeriesLabel};

use super::card::{SWATCH, swatch};
use crate::theme::{ExtraColor, ExtraRadius};

/// The stripes across a hatched part: how many fit its diagonal, each 35% of its own band.
const STRIPES: usize = 3;
const STRIPE_SHARE: f32 = 0.35;

/// The color of the palette's place `index` (`ProviderSeries::index`): its first three, then Other's.
fn palette(index: usize) -> PaletteColor {
  match index {
    0 => ExtraColor::Chart1.role(),
    1 => ExtraColor::Chart2.role(),
    2 => ExtraColor::Chart3.role(),
    _ => ExtraColor::ChartOther.role(),
  }
}

/// The color `series` is drawn in; Earlier runs' stripes and outline.
pub fn series_color(series: ProviderSeries) -> PaletteColor {
  match series {
    ProviderSeries::Earlier => ExtraColor::ChartNone.role(),
    series => palette(series.index()),
  }
}

/// `series`' swatch (`Swatch` `p4TazJ`): Earlier runs' hatched (`LEQ1w`).
pub fn series_swatch(series: ProviderSeries) -> Element {
  match series {
    ProviderSeries::Earlier => hatched(Row::new().size(Dimension::Px(SWATCH), Dimension::Px(SWATCH)))
      .flex_shrink(0.0)
      .rounded(ExtraRadius::Swatch.role())
      .into(),
    series => swatch(series_color(series)),
  }
}

/// `part` filled as `series` is: its color, or Earlier runs' stripes over `chart_hatch`.
pub fn series_fill(part: Row, series: ProviderSeries) -> Row {
  match series {
    ProviderSeries::Earlier => hatched(part),
    series => part.background(series_color(series)),
  }
}

/// The color of an account's plan limit lines.
pub fn account_color(series: AccountSeries) -> PaletteColor {
  palette(series.index())
}

/// The swatch of an account's plan limit lines.
pub fn account_swatch(series: AccountSeries) -> Element {
  swatch(account_color(series))
}

/// `part` hatched: `chart_none` stripes at 45° and a 1 outline over `chart_hatch` (`LEQ1w`).
fn hatched(part: Row) -> Row {
  let (ground, stripe) = (ExtraColor::ChartHatch.role(), ExtraColor::ChartNone.role());
  let band = 1.0 / STRIPES as f32;
  let stops = (0..STRIPES).flat_map(|index| {
    let start = index as f32 * band;
    let end = start + band * STRIPE_SHARE;
    [
      GradientStop::at(stripe.clone(), start),
      GradientStop::at(stripe.clone(), end),
      GradientStop::at(ground.clone(), end),
      GradientStop::at(ground.clone(), start + band),
    ]
  });
  part
    // At 135° the bands run as `/` does (`LEQ1w`).
    .background_gradient(Gradient::linear(135.0, stops.collect::<Vec<_>>()))
    .border(Border::inside(BorderSize::Sm, stripe))
}

/// A CLI's short name, as an account's label starts with it (*Codex*, *Claude*).
pub fn cli_short(ctx: &Ctx, cli: AccountCli) -> Arc<str> {
  ctx.t(match cli {
    AccountCli::Codex => "usage.cli.codex",
    AccountCli::ClaudeCode => "usage.cli.claude",
  })
}

/// An account's name as Settings shows it: its own, or *Default*.
pub fn account_name(ctx: &Ctx, name: Option<&Arc<str>>) -> Arc<str> {
  name.cloned().unwrap_or_else(|| ctx.t("usage.filters.defaultAccount"))
}

/// *Codex · Client*; an account no longer connected reads as such.
pub fn account_text(ctx: &Ctx, label: &AccountLabel) -> Arc<str> {
  match label {
    AccountLabel::Known { cli, name } => ctx.t_args(
      "usage.accounts.label",
      [
        ("cli", &*cli_short(ctx, *cli)),
        ("account", &*account_name(ctx, name.as_ref())),
      ],
    ),
    AccountLabel::Removed => ctx.t("usage.accounts.removed"),
  }
}

/// An agent provider by its name as the Connections page shows it; one gone since by the name it had, *(removed)*.
pub fn provider_text(ctx: &Ctx, label: &ProviderLabel) -> Arc<str> {
  match label {
    ProviderLabel::Connected { name } => name.clone(),
    ProviderLabel::Removed { name: Some(name) } => ctx.t_args("usage.providers.removed", [("name", &**name)]),
    ProviderLabel::Removed { name: None } => ctx.t("usage.providers.removedUnnamed"),
  }
}

/// How a series reads in a legend, a tooltip or a menu.
pub fn series_text(ctx: &Ctx, label: &SeriesLabel) -> Arc<str> {
  match label {
    SeriesLabel::Provider(provider) | SeriesLabel::OtherOne(provider) => provider_text(ctx, provider),
    SeriesLabel::Other(count) => ctx.t_args("usage.providers.other", [("count", count.to_string().as_str())]),
    SeriesLabel::Earlier => ctx.t("usage.providers.earlier"),
  }
}

/// How a series heads a table's column, in capitals: the provider's name (*CLAUDE · DEEPSEEK*), *OTHER* over
/// *2 PROVIDERS*, or *EARLIER RUNS*.
pub fn series_heading(ctx: &Ctx, label: &SeriesLabel) -> (Option<Arc<str>>, Arc<str>) {
  let upper = |text: Arc<str>| -> Arc<str> { text.to_uppercase().into() };
  match label {
    SeriesLabel::Other(count) => (
      Some(upper(ctx.t("usage.providers.otherHeading"))),
      upper(ctx.t_args("usage.providers.count", [("count", count.to_string().as_str())])),
    ),
    label => (None, upper(series_text(ctx, label))),
  }
}
