//! The head of the runs-on picker's Models column (`Connection` `D2vjC` in `v6mWp8`, `DBsz1` in `05b · On DeepSeek`
//! `AZp3I`): the connection shown, 2 over, 6 under and 8 at its sides, 10 apart: its name (12.5/500) over
//! *CLI · account · plan* (11.5 `text_muted`; a limit reached and until when in `warning`), then 88 wide its plan's
//! 5-hour use (Geist Mono 10.5) over a 4 meter, 3 apart. An API provider's says its CLI and host, the tokens its agents
//! spent today and *no plan limit*; its key's secret is their tooltip.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, text_style::TextAlign},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{connection::BaseUrl, id::ConnectionId, time::Timestamp, tokens::compact_tokens};
use orchester_workspace::{display, orchestrator::runs_on::RunsOn};

use super::rail::{Entry, Sources, cli_name};
use crate::{
  services::accounts::plan_name,
  theme::{ExtraSpacing, ExtraTypography, OrchestratorText},
  ui::{
    meter::{MeterTone, meter},
    tooltip::Tooltip,
  },
};

/// The plan's column's width (`CA20p`).
const PLAN_WIDTH: f32 = 88.0;
/// The 5-hour window, in minutes.
const FIVE_HOURS: u64 = 300;

/// The head of `entry`'s models.
pub(super) fn header(ctx: &mut Ctx, sources: &Sources<'_>, entry: &Entry) -> Element {
  let (who, warns, aside) = match &entry.runs_on {
    RunsOn::ApiProvider { base_url, .. } => {
      let (who, aside) = provider(ctx, sources, (entry, base_url));
      (who, false, Some(aside))
    }
    RunsOn::SignIn(_) | RunsOn::Native => sign_in(ctx, sources, entry),
  };
  let who = Text::new(&who)
    .id("orchestrator-runs-on-who")
    .variant(ExtraTypography::Note.role())
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0));
  Row::new()
    .id("orchestrator-runs-on-header")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Tight.role())
        .bottom(ExtraSpacing::Snug.role())
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm),
    )
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&entry.name)
            .id(format!("orchestrator-runs-on-{}-name", entry.id))
            .variant(OrchestratorText::Emphasis.role())
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        )
        .child(if warns { who.color(PaletteColor::Warning) } else { who }),
    )
    .with_children(aside)
    .into()
}

/// A sign-in's line under its name, whether it warns, and its plan's use.
fn sign_in(ctx: &Ctx, sources: &Sources<'_>, entry: &Entry) -> (Arc<str>, bool, Option<Element>) {
  let cli = cli_name(ctx, entry.provider);
  let who = sources.signed_in(&entry.runs_on, entry.provider);
  if !who.signed_in {
    return (
      ctx.t_args("orchestrator.runsOn.signedOut", [("cli", &*cli)]),
      false,
      None,
    );
  }
  let now = Timestamp::now();
  let reached = who.reached(now);
  let identity = who
    .account
    .and_then(|account| account.last_check())
    .and_then(|check| check.known_identity());
  // Who is signed in is not known yet: the connection's name tells two accounts of one name apart.
  let email: Arc<str> = identity
    .and_then(|identity| identity.email.as_deref())
    .filter(|email| !email.is_empty())
    .map_or_else(|| connection_name(sources, entry.id), Arc::from);
  let until = who.reading.and_then(|reading| reading.resets_at(now));
  let line = match (reached, until, identity.and_then(|identity| identity.plan.as_deref())) {
    (true, Some(until), _) => ctx.t_args(
      "orchestrator.runsOn.limitUntil",
      [("cli", &*cli), ("email", &*email), ("time", &display::clock(until))],
    ),
    (_, _, Some(plan)) => ctx.t_args(
      "orchestrator.runsOn.who",
      [("cli", &*cli), ("email", &*email), ("plan", &plan_name(plan))],
    ),
    _ => ctx.t_args("orchestrator.runsOn.whoNoPlan", [("cli", &*cli), ("email", &*email)]),
  };
  let used = who.reading.and_then(|reading| {
    reading
      .limits
      .windows
      .iter()
      .find(|window| window.minutes() == FIVE_HOURS && !window.expired(now))
      .and_then(|window| window.percent())
  });
  let plan = plan_column(ctx, &format!("orchestrator-runs-on-{}", entry.id), (used, reached));
  (line, reached, Some(plan))
}

/// The name of the connection `id`.
fn connection_name(sources: &Sources<'_>, id: ConnectionId) -> Arc<str> {
  sources
    .setup
    .connections()
    .into_iter()
    .find(|choice| choice.id == id)
    .map_or_else(|| Arc::from(""), |choice| choice.name)
}

/// An API provider's line under its name and its tokens today, its key's secret in their tooltip.
fn provider(ctx: &mut Ctx, sources: &Sources<'_>, (entry, base_url): (&Entry, &BaseUrl)) -> (Arc<str>, Element) {
  let id = entry.id;
  let used = sources.uses.iter().find(|used| used.connection == id);
  let who = ctx.t_args(
    "orchestrator.runsOn.providerWho",
    [("cli", &*cli_name(ctx, entry.provider)), ("host", base_url.host())],
  );
  let tokens = used.map_or(0, |used| used.tokens);
  let usage = Column::new()
    .id(format!("orchestrator-runs-on-{id}-usage"))
    .min_width(Dimension::Px(PLAN_WIDTH))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Fine.role())
    .align_items(Alignment::End)
    .child(
      Text::new(&ctx.t_args("orchestrator.runsOn.today", [("tokens", compact_tokens(tokens))]))
        .id(format!("orchestrator-runs-on-{id}-tokens"))
        .variant(OrchestratorText::PlanUse.role())
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.noPlanLimit"))
        .variant(OrchestratorText::PlanUse.role())
        .nowrap(),
    );
  let tip = match used.and_then(|used| used.key.as_deref()) {
    Some(key) => ctx.t_args("orchestrator.runsOn.key", [("secret", key)]),
    None => ctx.t("orchestrator.runsOn.keyGone"),
  };
  let usage = Tooltip::new(format!("orchestrator-runs-on-{id}-key"), tip).wrap(ctx, usage);
  (who, usage)
}

/// A plan's 5-hour use: `42% of 5h` over its meter, or *limit reached* in `warning`; `id` names it.
fn plan_column(ctx: &Ctx, id: &str, (used, reached): (Option<f64>, bool)) -> Element {
  let (words, tone, color) = if reached {
    (
      ctx.t("orchestrator.runsOn.limitReached"),
      MeterTone::Reached,
      PaletteColor::Warning,
    )
  } else {
    let percent = used.map(|used| used.round() as u64);
    let words = percent.map_or_else(
      || ctx.t("orchestrator.runsOn.useUnknown"),
      |percent| ctx.t_args("orchestrator.runsOn.use", [("used", percent.to_string())]),
    );
    (words, MeterTone::Plain, PaletteColor::TextMuted)
  };
  // A share of at most a hundred: whole ones are what the meter shows.
  let share = if reached {
    100
  } else {
    used.map_or(0, |used| used.round() as u64)
  };
  // As wide as its words when they are longer, so none is cut; the meter's track is 88 wide at its end (`Nlcbo`).
  Column::new()
    .id(format!("{id}-plan"))
    .min_width(Dimension::Px(PLAN_WIDTH))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Fine.role())
    .align_items(Alignment::End)
    .child(
      Text::new(&words)
        .variant(OrchestratorText::PlanUse.role())
        .color(color)
        .text_align(TextAlign::Right)
        .nowrap(),
    )
    .child(
      Row::new()
        .width(Dimension::Px(PLAN_WIDTH))
        .flex_shrink(0.0)
        .child(meter(&format!("{id}-meter"), (share, 100), tone)),
    )
    .into()
}
