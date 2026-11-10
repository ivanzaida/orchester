//! What the runs-on picker is read from: the project's connections the Orchestrator can run on, Claude Code's
//! sign-ins, then Codex's, then the API providers, each with who it runs as and whether it can be chosen.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
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

use crate::services::accounts::{Accounts, account_name};

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
  pub(super) limit: bool,
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
