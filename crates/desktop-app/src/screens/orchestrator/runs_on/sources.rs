//! What the runs-on picker is read from: this computer's accounts, as the Orchestrator can run on them. Claude Code's
//! sign-ins, then Codex's, then the API providers: the project's connections it can run on, each account once (an
//! account is who is signed in, so the connections on the folders of one sign-in are one item), and the accounts the
//! project has no connection on yet ([`Open`]), which get one when they are chosen
//! (`docs/design/agent-accounts.md`, "Agents run on accounts").

use std::sync::Arc;

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_core::{
  account::{AccountState, AgentAccount, ApiAccount, ApiSetup, by_sign_in},
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, LimitState, PlanReadings},
  },
  id::{AgentAccountId, ApiAccountId, ConnectionId},
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
  /// The person's API accounts.
  pub(super) api: &'a [ApiAccount],
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
pub(super) enum Group {
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
  pub(super) group: Group,
}

/// An account of the person's: a sign-in, or an API account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Account {
  SignIn(AgentAccountId),
  Api(ApiAccountId),
}

impl DevtoolsInspectable for Account {}

/// An account on the rail the project has no connection on yet: choosing it makes the connection, and the Orchestrator
/// runs on it.
pub(super) struct Open {
  pub(super) account: Account,
  pub(super) provider: AgentProvider,
  pub(super) name: Arc<str>,
  /// An API provider's host.
  pub(super) host: Option<Arc<str>>,
  pub(super) choosable: bool,
  pub(super) group: Group,
}

impl Open {
  /// What the rail's item is called.
  pub(super) fn id(&self) -> String {
    match self.account {
      Account::SignIn(id) => format!("account-{id}"),
      Account::Api(id) => format!("api-{id}"),
    }
  }
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
    self.who(self.account(runs_on, provider))
  }

  /// Who the sign-in `account` runs as.
  pub(super) fn who<'a>(&'a self, account: Option<&'a AgentAccount>) -> SignedIn<'a> {
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
    self.once_each(entries)
  }

  /// `entries` with each sign-in once: the connections on the folders of one sign-in are one item, the one the
  /// Orchestrator runs on, else the first.
  fn once_each(&self, entries: Vec<Entry>) -> Vec<Entry> {
    let chosen = self.setup.settings.as_ref().map(|settings| settings.connection());
    let mut kept: Vec<(Option<(AgentProvider, usize)>, Entry)> = Vec::new();
    for entry in entries {
      let key = self
        .account(&entry.runs_on, entry.provider)
        .and_then(|account| Some((entry.provider, self.sign_in(entry.provider, account)?)));
      match kept.iter_mut().find(|(other, _)| key.is_some() && *other == key) {
        Some(same) if Some(entry.id) == chosen => same.1 = entry,
        Some(_) => {}
        None => kept.push((key, entry)),
      }
    }
    kept.into_iter().map(|(_, entry)| entry).collect()
  }

  /// Which sign-in of `provider`'s CLI `account` is: the folders one email is signed in to are one.
  fn sign_in(&self, provider: AgentProvider, account: &AgentAccount) -> Option<usize> {
    by_sign_in(self.accounts.of_provider(provider))
      .iter()
      .position(|folders| folders.iter().any(|folder| folder.id() == account.id()))
  }

  /// The person's accounts the project has no connection on, in the rail's groups: each sign-in once, then the API
  /// providers of an agent CLI.
  pub(super) fn open(&self, ctx: &Ctx) -> Vec<Open> {
    let on: Vec<(AgentProvider, RunsOn)> = self
      .setup
      .connections()
      .into_iter()
      .filter_map(|choice| Some((choice.provider, self.setup.runs_on(choice.id)?)))
      .collect();
    let connected = |account: &AgentAccount| {
      on.iter().any(|(provider, runs_on)| {
        self
          .account(runs_on, *provider)
          .is_some_and(|found| found.id() == account.id())
      })
    };
    let mut open = Vec::new();
    for (provider, group) in [
      (AgentProvider::ClaudeCodeCli, Group::Claude),
      (AgentProvider::CodexCli, Group::Codex),
    ] {
      for folders in by_sign_in(self.accounts.of_provider(provider)) {
        let Some(account) = folders
          .first()
          .filter(|_| !folders.iter().any(|folder| connected(folder)))
        else {
          continue;
        };
        open.push(Open {
          account: Account::SignIn(account.id()),
          provider,
          name: account_name(ctx, account).into(),
          host: None,
          choosable: self.who(Some(account)).signed_in,
          group,
        });
      }
    }
    for account in self.api {
      let ApiSetup::Cli { cli, base_url, .. } = account.setup() else {
        continue;
      };
      // The project's connection on an API account names the account's key, which has the account's id.
      let kept = on.iter().any(
        |(_, runs_on)| matches!(runs_on, RunsOn::ApiProvider { key, .. } if key.as_uuid() == account.id().as_uuid()),
      );
      if !kept {
        open.push(Open {
          account: Account::Api(account.id()),
          provider: cli.provider(),
          name: account.name().as_str().into(),
          host: Some(base_url.host().into()),
          choosable: true,
          group: Group::Providers,
        });
      }
    }
    open
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

/// What the tooltip of an account the project has no connection on says: its name, and why it cannot be chosen.
pub(super) fn tip_open(ctx: &Ctx, open: &Open) -> Arc<str> {
  match &open.host {
    Some(host) => ctx.t_args(
      "orchestrator.runsOn.railProvider",
      [("name", &*open.name), ("host", &**host)],
    ),
    None => {
      let key = if open.choosable {
        "orchestrator.runsOn.railSignIn"
      } else {
        "orchestrator.runsOn.railSignedOut"
      };
      ctx.t_args(key, [("cli", &*cli_name(ctx, open.provider)), ("name", &*open.name)])
    }
  }
}
