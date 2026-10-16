//! How a series looks (`Chart colors` `FDC8B`, `Series hatch` `LEQ1w`): accounts 1–3 in `chart_1`–`chart_3`,
//! Other accounts in `chart_other`, No account hatched — `chart_none` stripes and outline over `chart_hatch` —
//! and the words of its label (*Codex · Client*, *Other accounts · 2*, *No account*).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor},
  },
  components::Row,
  node::{Element, Gradient, GradientStop, border::Border, dimension::Dimension},
};
use orchester_core::{account::AccountCli, usage::AccountSeries};
use orchester_workspace::usage::{AccountLabel, SeriesLabel};

use super::card::{SWATCH, swatch};
use crate::theme::{ExtraColor, ExtraRadius};

/// The stripes across a hatched part: how many fit its diagonal, each 35% of its own band.
const STRIPES: usize = 3;
const STRIPE_SHARE: f32 = 0.35;

/// The color `series` is drawn in; No account's stripes and outline.
pub fn series_color(series: AccountSeries) -> PaletteColor {
  match series {
    AccountSeries::Account(place) => match place.get() {
      0 => ExtraColor::Chart1.role(),
      1 => ExtraColor::Chart2.role(),
      _ => ExtraColor::Chart3.role(),
    },
    AccountSeries::Other => ExtraColor::ChartOther.role(),
    AccountSeries::NoAccount => ExtraColor::ChartNone.role(),
  }
}

/// `series`' swatch (`Swatch` `p4TazJ`): No account's hatched (`LEQ1w`).
pub fn series_swatch(series: AccountSeries) -> Element {
  match series {
    AccountSeries::NoAccount => hatched(Row::new().size(Dimension::Px(SWATCH), Dimension::Px(SWATCH)))
      .flex_shrink(0.0)
      .rounded(ExtraRadius::Swatch.role())
      .into(),
    series => swatch(series_color(series)),
  }
}

/// `part` filled as `series` is: its color, or No account's stripes over `chart_hatch`.
pub fn series_fill(part: Row, series: AccountSeries) -> Row {
  match series {
    AccountSeries::NoAccount => hatched(part),
    series => part.background(series_color(series)),
  }
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

/// A CLI's short name, as a series label starts with it (*Codex*, *Claude*).
pub fn cli_short(ctx: &Ctx, cli: AccountCli) -> Arc<str> {
  ctx.t(match cli {
    AccountCli::Codex => "usage.cli.codex",
    AccountCli::ClaudeCode => "usage.cli.claude",
  })
}

/// A CLI's name as a menu heads its accounts (*Codex*, *Claude Code*).
pub fn cli_heading(ctx: &Ctx, cli: AccountCli) -> Arc<str> {
  ctx.t(match cli {
    AccountCli::Codex => "usage.accounts.codex",
    AccountCli::ClaudeCode => "usage.accounts.claudeCode",
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

/// How a series reads in a legend, a tooltip or a menu.
pub fn series_text(ctx: &Ctx, label: &SeriesLabel) -> Arc<str> {
  match label {
    SeriesLabel::Account(account) | SeriesLabel::OtherOne(account) => account_text(ctx, account),
    SeriesLabel::Other(count) => ctx.t_args("usage.accounts.other", [("count", count.to_string().as_str())]),
    SeriesLabel::NoAccount => ctx.t("usage.accounts.none"),
  }
}

/// How a series heads a table's column, in capitals: the CLI over the account (*CODEX* / *CLIENT*), *OTHER* over
/// *2 ACCOUNTS*, or *NO ACCOUNT* alone.
pub fn series_heading(ctx: &Ctx, label: &SeriesLabel) -> (Option<Arc<str>>, Arc<str>) {
  let upper = |text: Arc<str>| -> Arc<str> { text.to_uppercase().into() };
  match label {
    SeriesLabel::Account(AccountLabel::Known { cli, name })
    | SeriesLabel::OtherOne(AccountLabel::Known { cli, name }) => (
      Some(upper(cli_short(ctx, *cli))),
      upper(account_name(ctx, name.as_ref())),
    ),
    SeriesLabel::Account(AccountLabel::Removed) | SeriesLabel::OtherOne(AccountLabel::Removed) => {
      (None, upper(ctx.t("usage.accounts.removed")))
    }
    SeriesLabel::Other(count) => (
      Some(upper(ctx.t("usage.accounts.otherHeading"))),
      upper(ctx.t_args("usage.accounts.count", [("count", count.to_string().as_str())])),
    ),
    SeriesLabel::NoAccount => (None, upper(ctx.t("usage.accounts.none"))),
  }
}
