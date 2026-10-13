//! The person's agent CLI accounts, for the screens that name a connection's account (Connections, agent profiles,
//! pipeline steps, a run's summary). Read once when such a screen shows; Settings › Accounts reads its own.

use std::sync::Arc;

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_core::{
  account::AgentAccount,
  connection::{AgentProvider, ConnectionAccount},
  id::AgentAccountId,
};
use orchester_workspace::{background, messages};

use super::Services;

/// The accounts as read; empty until they are, or when they can't be.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Accounts(pub Arc<Vec<AgentAccount>>);

impl DevtoolsInspectable for Accounts {}

impl Accounts {
  pub fn of(&self, id: AgentAccountId) -> Option<&AgentAccount> {
    self.0.iter().find(|account| account.id() == id)
  }

  /// The account a connection of `provider` runs on; `None` without one, or for a native connection.
  pub fn of_connection(&self, account: &ConnectionAccount, provider: AgentProvider) -> Option<&AgentAccount> {
    self.of(account.id(provider)?)
  }

  /// The accounts of `provider`'s CLI, Default first.
  pub fn of_provider(&self, provider: AgentProvider) -> impl Iterator<Item = &AgentAccount> {
    self
      .0
      .iter()
      .filter(move |account| account.cli().provider() == provider)
  }
}

/// The accounts, read when the calling screen shows.
pub fn listed(ctx: &mut Ctx) -> Accounts {
  let workspace = Services::of(ctx).workspace;
  ctx
    .future((), move |()| {
      let workspace = workspace.clone();
      async move {
        background::run(move || workspace.agent_accounts())
          .await?
          .map(|accounts| Accounts(Arc::new(accounts)))
          .map_err(|error| {
            tracing::warn!(%error, "the accounts can't be read for a connection's account");
            messages::store_error(&error)
          })
      }
    })
    .state()
    .get()
    .data
    .unwrap_or_default()
}
