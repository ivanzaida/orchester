//! The runs-on picker's rail (`Rail` `hM4ul` in `Runs on menu` `r2owi1`, `05 Account and model` `v6mWp8`): the
//! project's connections the Orchestrator can run on, Claude Code's sign-ins, then Codex's, then the API providers,
//! the groups split by a 20-wide `border` line 2 over and under it. 6 inside, 4 apart, a `border` stroke on its right:
//! each a 36 square, radius 6, with its maker's 16 mark, or for an API provider the 16 lucide cloud in
//! `text_secondary`. The chosen one on `pressed` with a 2 × 16 `accent` bar at the rail's edge, a plan at its limit
//! with a 7 `warning` dot ringed in `surface_raised` at its top right, an account not signed in at half and not to be
//! chosen. A tooltip names each (*Claude Code · Work*), and says why one cannot be chosen.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Column, Row, Stack},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  account::{AccountState, AgentAccount},
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, LimitState, PlanReadings},
  },
  id::ConnectionId,
  time::Timestamp,
};
use orchester_workspace::orchestrator::{
  runs_on::{ProviderUse, RunsOn},
  settings::OrchestratorSetup,
};

use crate::{
  services::accounts::{Accounts, account_name},
  theme::{DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing, IconSize},
  ui::{
    agent_mark::AgentMark,
    icon::{Icon, icon},
    pressable::{inert, pressable},
    tooltip::Tooltip,
  },
};

/// A rail item's side (`bIGvo`), the chosen bar's size (`ERvx7`) and the limit dot's (`KsrNe`), at 24, 6 in the item.
const ITEM: f32 = 36.0;
const BAR_WIDTH: f32 = 2.0;
const BAR_HEIGHT: f32 = 16.0;
const DOT: f32 = 7.0;
const DOT_LEFT: f32 = 24.0;
/// A group's line (`crhDx` in `OJPue`): 1 high, 8 in from the item's sides.
const LINE: f32 = 1.0;

/// What the picker is read from.
pub(super) struct Sources<'a> {
  pub(super) setup: &'a OrchestratorSetup,
  pub(super) accounts: &'a Accounts,
  pub(super) readings: &'a PlanReadings,
  pub(super) uses: &'a [ProviderUse],
}

/// Who a sign-in connection runs as: its account when it is known, whether it is signed in, and its plan's reading.
pub(super) struct SignedIn<'a> {
  pub(super) account: Option<&'a AgentAccount>,
  pub(super) signed_in: bool,
  pub(super) reading: Option<&'a LimitReading>,
}

impl SignedIn<'_> {
  /// The plan's limit is reached now.
  pub(super) fn reached(&self, now: Timestamp) -> bool {
    self
      .reading
      .is_some_and(|reading| reading.state(now) == LimitState::Reached)
  }
}

/// A group of the rail, in the rail's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
  Claude,
  Codex,
  Providers,
}

/// A connection on the rail.
pub(super) struct Entry {
  pub(super) id: ConnectionId,
  pub(super) provider: AgentProvider,
  pub(super) runs_on: RunsOn,
  /// Its account's name, else the connection's.
  pub(super) name: Arc<str>,
  pub(super) choosable: bool,
  limit: bool,
  group: Group,
}

impl Sources<'_> {
  /// The account a connection of `provider` on `runs_on` runs on.
  pub(super) fn account(&self, runs_on: &RunsOn, provider: AgentProvider) -> Option<&AgentAccount> {
    match runs_on {
      RunsOn::SignIn(account) => self.accounts.of_connection(account, provider),
      RunsOn::ApiProvider { .. } | RunsOn::Native => None,
    }
  }

  /// Who a connection of `provider` on `runs_on` runs as.
  pub(super) fn signed_in(&self, runs_on: &RunsOn, provider: AgentProvider) -> SignedIn<'_> {
    let account = self.account(runs_on, provider);
    let signed_in = account.is_some_and(|account| {
      matches!(
        account.state(),
        AccountState::SignedIn | AccountState::Unknown | AccountState::NotChecked
      )
    });
    let reading = account
      .filter(|_| signed_in)
      .and_then(|account| self.readings.of_account(account.id()));
    SignedIn {
      account,
      signed_in,
      reading,
    }
  }

  /// The connections the Orchestrator can run on, by group: Claude Code's sign-ins, Codex's, the API providers.
  pub(super) fn entries(&self, ctx: &Ctx) -> Vec<Entry> {
    let now = Timestamp::now();
    let mut entries: Vec<Entry> = self
      .setup
      .connections()
      .into_iter()
      .filter(|choice| choice.unavailable.is_none())
      .filter_map(|choice| {
        let runs_on = self.setup.runs_on(choice.id)?;
        let (group, name, choosable, limit) = match &runs_on {
          RunsOn::Native => return None,
          RunsOn::ApiProvider { .. } => (Group::Providers, choice.name.clone(), true, false),
          RunsOn::SignIn(_) => {
            let who = self.signed_in(&runs_on, choice.provider);
            let name = who
              .account
              .map_or_else(|| choice.name.clone(), |account| account_name(ctx, account).into());
            let group = match choice.provider {
              AgentProvider::CodexCli => Group::Codex,
              _ => Group::Claude,
            };
            (group, name, who.signed_in, who.reached(now))
          }
        };
        Some(Entry {
          id: choice.id,
          provider: choice.provider,
          runs_on,
          name,
          choosable,
          limit,
          group,
        })
      })
      .collect();
    entries.sort_by_key(|entry| entry.group);
    entries
  }
}

/// The CLI's name as the picker says it: *Claude Code*, *Codex*.
pub(super) fn cli_name(ctx: &Ctx, provider: AgentProvider) -> Arc<str> {
  match provider {
    AgentProvider::CodexCli => ctx.t("orchestrator.runsOn.codex"),
    _ => ctx.t("orchestrator.runsOn.claude"),
  }
}

/// The rail of `entries`, `chosen` on `pressed`, the keys at `cursor`; `choose` takes an entry's connection.
pub(super) fn rail(
  ctx: &mut Ctx,
  entries: &[Entry],
  (chosen, cursor): (Option<ConnectionId>, Option<usize>),
  choose: &Arc<dyn Fn(ConnectionId) + Send + Sync>,
) -> Element {
  let mut items: Vec<Element> = Vec::new();
  for (index, entry) in entries.iter().enumerate() {
    if index > 0 && entries[index - 1].group != entry.group {
      items.push(line());
    }
    let state = (chosen == Some(entry.id), cursor == Some(index));
    items.push(item(ctx, entry, state, choose));
  }
  Column::new()
    .id("orchestrator-runs-on-rail")
    .flex_shrink(0.0)
    // Its items reach its start, where the chosen one's bar is (`ERvx7`): 6 in on the other sides only.
    .padding(
      Padding::new()
        .top(ExtraSpacing::Snug.role())
        .right(ExtraSpacing::Snug.role())
        .bottom(ExtraSpacing::Snug.role()),
    )
    .spacing(SpacingSize::Xs)
    .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(items)
    .into()
}

/// The line between two groups.
fn line() -> Element {
  let line = Column::new()
    .width(Dimension::Px(ITEM))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Tight.role()))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(LINE))
        .background(PaletteColor::Border),
    );
  Row::new()
    .padding(Padding::new().left(ExtraSpacing::Snug.role()))
    .child(line)
    .into()
}

/// An entry's item, `chosen` and where the keys are (`current`) as said.
fn item(
  ctx: &mut Ctx,
  entry: &Entry,
  (chosen, current): (bool, bool),
  choose: &Arc<dyn Fn(ConnectionId) + Send + Sync>,
) -> Element {
  let id = entry.id;
  let mark = match entry.runs_on {
    RunsOn::ApiProvider { .. } => icon(Icon::Cloud, IconSize::Lg, PaletteColor::TextSecondary).into(),
    RunsOn::SignIn(_) | RunsOn::Native => AgentMark::of(entry.provider).render(ctx, IconSize::Lg),
  };
  let tip = tip(ctx, entry);
  let button = if entry.choosable {
    let choose = choose.clone();
    pressable(move || choose(id)).hovered_style(Style::new().background(ExtraColor::Hover.role()))
  } else {
    inert(Button::empty()).opacity(DISABLED_OPACITY)
  };
  let button = button
    .id(format!("orchestrator-runs-on-{id}"))
    .describe("chosen", chosen.to_string())
    .describe("choosable", entry.choosable.to_string())
    .describe("limit", entry.limit.to_string())
    .describe("a11y_name", tip.to_string())
    .width(Dimension::Px(ITEM))
    .height(Dimension::Px(ITEM))
    .justify(Justify::Center)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(mark);
  let button = match (chosen, current) {
    (true, _) => button.background(ExtraColor::Pressed.role()),
    (false, true) => button.background(ExtraColor::Hover.role()),
    (false, false) => button,
  };
  // A stack, as only a stack lays out what is placed at a point of it: the dot, and the bar in the 6 before the item at
  // the rail's start, inside the stack so that nothing around it clips it.
  let item = Stack::new()
    .id(format!("orchestrator-runs-on-{id}-item"))
    .child(
      Row::new()
        .padding(Padding::new().left(ExtraSpacing::Snug.role()))
        .child(button),
    )
    .with_children(entry.limit.then(|| limit_dot(id)))
    .with_children(chosen.then(|| chosen_bar(id)));
  Tooltip::new(format!("orchestrator-runs-on-{id}-tip"), tip).wrap(ctx, item)
}

/// What an entry's tooltip says: its name, and why it cannot be chosen.
pub(super) fn tip(ctx: &Ctx, entry: &Entry) -> Arc<str> {
  match &entry.runs_on {
    RunsOn::ApiProvider { base_url, .. } => ctx.t_args(
      "orchestrator.runsOn.railProvider",
      [("name", &*entry.name), ("host", base_url.host())],
    ),
    RunsOn::SignIn(_) | RunsOn::Native => {
      let key = if entry.choosable {
        "orchestrator.runsOn.railSignIn"
      } else {
        "orchestrator.runsOn.railSignedOut"
      };
      ctx.t_args(key, [("cli", &*cli_name(ctx, entry.provider)), ("name", &*entry.name)])
    }
  }
}

/// The `warning` dot of a plan at its limit, at the item's top right.
fn limit_dot(id: ConnectionId) -> Element {
  Row::new()
    .id(format!("orchestrator-runs-on-{id}-limit"))
    .absolute(
      ExtraSpacing::Snug.value() + DOT_LEFT,
      ExtraSpacing::Snug.value(),
      Dimension::Px(DOT),
      Dimension::Px(DOT),
    )
    .pointer_events_none()
    .rounded(ExtraRadius::Full.role())
    .background(PaletteColor::Warning)
    .border(Border::inside(BorderSize::Md, PaletteColor::SurfaceRaised))
    .into()
}

/// The `accent` bar of the chosen item, at the rail's edge.
fn chosen_bar(id: ConnectionId) -> Element {
  Row::new()
    .id(format!("orchestrator-runs-on-{id}-bar"))
    .absolute(
      0.0,
      (ITEM - BAR_HEIGHT) / 2.0,
      Dimension::Px(BAR_WIDTH),
      Dimension::Px(BAR_HEIGHT),
    )
    .pointer_events_none()
    .rounded(ExtraRadius::Full.role())
    .background(PaletteColor::Accent)
    .into()
}
