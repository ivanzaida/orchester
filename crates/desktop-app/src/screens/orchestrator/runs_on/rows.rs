//! The runs-on picker's rows (`Runs on menu` `WaQcn` in `05 Account and model picker` `v6mWp8`): the project's agent
//! connections the Orchestrator can run on, grouped under a 26-high heading (the CLI's maker's 12 mark, or for API
//! providers the 12 lucide cloud, and 11/500 `text_muted`):
//! *Claude Code · your sign-ins*, *Codex · your sign-ins*, then *API providers · pay per token*. A row is 45 high,
//! radius 6, 6 by 8 inside, 10 apart: its name (12.5/500) over who is signed in and the plan (11.5 `text_muted`; a
//! limit reached and until when in `warning`; the connection while who is unknown), then at least 88 wide the plan's
//! 5-hour use (Geist Mono 10.5) over a 4 meter, and
//! a check on the chosen one on the `hover` fill. An account that is not signed in is faded, says to sign in in
//! Connections and cannot be chosen. An API provider's row says its CLI and host, the tokens its agents spent today
//! and *no plan limit*; its key's secret is its tooltip.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, text_style::TextAlign},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  account::{AccountState, AgentAccount},
  connection::{
    AgentProvider, BaseUrl,
    invocation::Platform,
    plan_limits::{LimitState, PlanReadings},
  },
  id::ConnectionId,
  time::Timestamp,
  tokens::compact_tokens,
};
use orchester_workspace::{
  display,
  orchestrator::{
    runs_on::{ProviderUse, RunsOn},
    settings::{ConnectionChoice, OrchestratorSetup},
  },
};

use crate::{
  services::accounts::{Accounts, account_name, plan_name},
  theme::{DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize, OrchestratorText},
  ui::{
    agent_mark::agent_mark,
    icon::{Icon, icon},
    meter::{MeterTone, meter},
    pressable::{inert, pressable},
    tooltip::Tooltip,
  },
};

/// A row's height (`w901t`) and its plan column's width (`xiHm1`).
const ROW_HEIGHT: f32 = 45.0;
const PLAN_WIDTH: f32 = 88.0;
/// A heading's height (`VEhGk`).
const HEADING_HEIGHT: f32 = 26.0;
/// The 5-hour window, in minutes.
const FIVE_HOURS: u64 = 300;

/// What the rows are read from.
pub(super) struct Sources<'a> {
  pub(super) setup: &'a OrchestratorSetup,
  pub(super) accounts: &'a Accounts,
  pub(super) readings: &'a PlanReadings,
  pub(super) uses: &'a [ProviderUse],
}

/// The connection a row stands for and how it reads.
pub(super) struct Shown {
  pub(super) connection: ConnectionChoice,
  pub(super) runs_on: RunsOn,
}

impl Sources<'_> {
  /// The connections the Orchestrator can run on, each with what it runs on.
  pub(super) fn shown(&self) -> Vec<Shown> {
    self
      .setup
      .connections()
      .into_iter()
      .filter(|choice| choice.unavailable.is_none())
      .filter_map(|connection| {
        let runs_on = self.setup.runs_on(connection.id)?;
        (runs_on != RunsOn::Native).then_some(Shown { connection, runs_on })
      })
      .collect()
  }

  /// The account a connection of `provider` on `runs_on` runs on.
  pub(super) fn account(&self, runs_on: &RunsOn, provider: AgentProvider) -> Option<&AgentAccount> {
    match runs_on {
      RunsOn::SignIn(account) => self.accounts.of_connection(account, provider),
      RunsOn::ApiProvider { .. } | RunsOn::Native => None,
    }
  }
}

/// The CLI's name as the picker says it: *Claude Code*, *Codex*.
pub(super) fn cli_name(ctx: &Ctx, provider: AgentProvider) -> Arc<str> {
  match provider {
    AgentProvider::CodexCli => ctx.t("orchestrator.runsOn.codex"),
    _ => ctx.t("orchestrator.runsOn.claude"),
  }
}

/// The picker's groups and their rows, `chosen` checked; `choose` takes a row's connection.
pub(super) fn groups(
  ctx: &mut Ctx,
  sources: &Sources<'_>,
  chosen: Option<ConnectionId>,
  choose: &Arc<dyn Fn(ConnectionId) + Send + Sync>,
) -> Vec<Element> {
  let shown = sources.shown();
  let mut parts: Vec<Element> = Vec::new();
  for (provider, key) in [
    (AgentProvider::ClaudeCodeCli, "orchestrator.runsOn.claudeSignIns"),
    (AgentProvider::CodexCli, "orchestrator.runsOn.codexSignIns"),
  ] {
    let rows: Vec<Element> = shown
      .iter()
      .filter(|row| row.connection.provider == provider && matches!(row.runs_on, RunsOn::SignIn(_)))
      .map(|row| sign_in_row(ctx, sources, row, (chosen, choose)))
      .collect();
    if !rows.is_empty() {
      // The CLI's maker's mark (`Mark` `PDVXx`, `FW7qZ`).
      let mark = agent_mark(ctx, provider, IconSize::Sm);
      parts.push(heading(
        ctx,
        &format!("orchestrator-runs-on-{}", key_suffix(provider)),
        (mark, key),
      ));
      parts.extend(rows);
    }
  }
  let providers: Vec<Element> = shown
    .iter()
    .filter_map(|row| match &row.runs_on {
      RunsOn::ApiProvider { base_url, .. } => Some(provider_row(ctx, sources, (row, base_url), (chosen, choose))),
      RunsOn::SignIn(_) | RunsOn::Native => None,
    })
    .collect();
  if !providers.is_empty() {
    parts.push(heading(
      ctx,
      "orchestrator-runs-on-providers",
      (
        icon(Icon::Cloud, IconSize::Sm, PaletteColor::TextMuted).into(),
        "orchestrator.runsOn.providers",
      ),
    ));
    parts.extend(providers);
  }
  parts
}

const fn key_suffix(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "codex",
    _ => "claude",
  }
}

/// A group's heading, named `id`: its glyph and its words.
fn heading(ctx: &Ctx, id: &str, (glyph, key): (Element, &str)) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADING_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(glyph)
    .child(
      Text::new(&ctx.t(key))
        .variant(lurq::app::theme::TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// A sign-in connection's row.
fn sign_in_row(
  ctx: &mut Ctx,
  sources: &Sources<'_>,
  row: &Shown,
  (chosen, choose): (Option<ConnectionId>, &Arc<dyn Fn(ConnectionId) + Send + Sync>),
) -> Element {
  let account = sources.account(&row.runs_on, row.connection.provider);
  let name: Arc<str> = account.map_or_else(
    || row.connection.name.clone(),
    |account| account_name(ctx, account).into(),
  );
  let signed_in = account.is_some_and(|account| {
    matches!(
      account.state(Platform::current()),
      AccountState::SignedIn | AccountState::Unknown | AccountState::NotChecked
    )
  });
  if !signed_in {
    let who = ctx.t("orchestrator.runsOn.signedOut");
    return unchoosable(frame(row.connection.id, (&name, who, false), None, None).opacity(DISABLED_OPACITY));
  }
  let now = Timestamp::now();
  let reading = account.and_then(|account| sources.readings.of_account(account.id()));
  let reached = reading.is_some_and(|reading| reading.state(now) == LimitState::Reached);
  let identity = account
    .and_then(|account| account.last_check())
    .and_then(|check| check.known_identity());
  let email = identity.and_then(|identity| identity.email.clone()).unwrap_or_default();
  let who = match (reached, reading.and_then(|reading| reading.resets_at(now))) {
    (true, Some(until)) => ctx.t_args(
      "orchestrator.runsOn.limitUntil",
      [("email", email.as_str()), ("time", &display::clock(until))],
    ),
    _ => match identity.and_then(|identity| identity.plan.as_deref()) {
      Some(plan) => ctx.t_args(
        "orchestrator.runsOn.who",
        [("email", email.as_str()), ("plan", &plan_name(plan))],
      ),
      None if !email.is_empty() => email.as_str().into(),
      // Who is signed in is not known yet: the connection tells two accounts of one name apart.
      None => row.connection.name.clone(),
    },
  };
  let used = reading.and_then(|reading| {
    reading
      .limits
      .windows
      .iter()
      .find(|window| window.minutes() == FIVE_HOURS && !window.expired(now))
      .and_then(|window| window.percent())
  });
  let plan = plan_column(
    ctx,
    &format!("orchestrator-runs-on-{}", row.connection.id),
    (used, reached),
  );
  let id = row.connection.id;
  let row = frame(id, (&name, who, reached), Some(plan), Some(chosen == Some(id)));
  let choose = choose.clone();
  pressable(move || choose(id))
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(row)
    .into()
}

/// The row of an API provider at `base_url`, its key's secret in its tooltip.
fn provider_row(
  ctx: &mut Ctx,
  sources: &Sources<'_>,
  (row, base_url): (&Shown, &BaseUrl),
  (chosen, choose): (Option<ConnectionId>, &Arc<dyn Fn(ConnectionId) + Send + Sync>),
) -> Element {
  let id = row.connection.id;
  let used = sources.uses.iter().find(|used| used.connection == id);
  let who = ctx.t_args(
    "orchestrator.runsOn.providerWho",
    [
      ("cli", &*cli_name(ctx, row.connection.provider)),
      ("host", base_url.host()),
    ],
  );
  let tokens = used.map_or(0, |used| used.tokens);
  let usage = Column::new()
    .width(Dimension::Px(PLAN_WIDTH))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::End)
    .child(
      Text::new(&ctx.t_args("orchestrator.runsOn.today", [("tokens", compact_tokens(tokens))]))
        .id(format!("orchestrator-runs-on-{id}-tokens"))
        .variant(OrchestratorText::PlanUse.role())
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.noPlanLimit"))
        .variant(OrchestratorText::PlanNote.role())
        .nowrap(),
    );
  let tip = match used.and_then(|used| used.key.as_deref()) {
    Some(key) => ctx.t_args("orchestrator.runsOn.key", [("secret", key)]),
    None => ctx.t("orchestrator.runsOn.keyGone"),
  };
  let shown = frame(
    id,
    (&row.connection.name, who, false),
    Some(usage.into()),
    Some(chosen == Some(id)),
  );
  let choose = choose.clone();
  let pressed = pressable(move || choose(id))
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(shown);
  Tooltip::new(format!("orchestrator-runs-on-{id}-key"), tip).wrap(ctx, pressed)
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
  // As wide as its words when they are longer, so none is cut.
  Column::new()
    .id(format!("{id}-plan"))
    .min_width(Dimension::Px(PLAN_WIDTH))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(&words)
        .variant(OrchestratorText::PlanUse.role())
        .color(color)
        .text_align(TextAlign::Right)
        .width(Dimension::Pct(100.0))
        .nowrap(),
    )
    .child(meter(&format!("{id}-meter"), (share, 100), tone))
    .into()
}

/// A row's frame: its name over `who` (in `warning` when `warns`), `aside` at its end and the check when `chosen` says
/// so; `None` for a row that cannot be chosen.
fn frame(
  id: ConnectionId,
  (name, who, warns): (&str, Arc<str>, bool),
  aside: Option<Element>,
  chosen: Option<bool>,
) -> Row {
  let who = Text::new(&who)
    .variant(ExtraTypography::Note.role())
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0));
  let row = Row::new()
    .id(format!("orchestrator-runs-on-{id}"))
    .describe("chosen", chosen.unwrap_or(false).to_string())
    .describe("choosable", chosen.is_some().to_string())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(name)
            .id(format!("orchestrator-runs-on-{id}-name"))
            .variant(OrchestratorText::Emphasis.role())
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        )
        .child(if warns { who.color(PaletteColor::Warning) } else { who }),
    )
    .with_children(aside)
    .with_children(
      chosen
        .filter(|chosen| *chosen)
        .map(|_| icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary)),
    );
  match chosen {
    Some(true) => row.background(ExtraColor::Hover.role()),
    Some(false) | None => row,
  }
}

/// A row that cannot be pressed.
pub(super) fn unchoosable(row: Row) -> Element {
  inert(lurq::components::Button::empty())
    .width(Dimension::Pct(100.0))
    .child(row)
    .into()
}
