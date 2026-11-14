//! What a profile can run on, as its one field offers it (`Agents on accounts` C1 `RwzH2`): every agent connection
//! of the project that runs agents, with who it runs as, the models it lists and the efforts each of them takes. The
//! window draws them on the rail of the *Runs on* picker.

use orchester_core::{
  account::AccountCli,
  connection::{AgentConnection, AgentProvider, ConnectionStatus},
  id::{AgentAccountId, ApiAccountId, ConnectionId},
  resource::ModelId,
};

use super::form::{effort_options, model_options};

/// A model a connection lists, with the reasoning efforts it takes there; none where the connection lists none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelChoice {
  pub id: String,
  pub efforts: Vec<String>,
}

/// A connection a local profile can run on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunsOnChoice {
  pub id: ConnectionId,
  pub name: String,
  pub cli: AgentProvider,
  /// The account its CLI is signed in to; `None` on an API provider or a model API, which run on a key.
  pub account: Option<AgentAccountId>,
  /// The host of the API it runs on instead of a sign-in.
  pub host: Option<String>,
  /// The API account an API provider's key is, by the key's id: the person's when the app has an account by this
  /// id, else a secret of the project's (a connection made before API accounts).
  pub api_account: Option<ApiAccountId>,
  /// Its last check found the CLI signed out.
  pub signed_out: bool,
  /// What it lists, in its order, with the profile's own model kept on the connection the profile runs on.
  pub models: Vec<ModelChoice>,
}

impl RunsOnChoice {
  /// Whether it lists `model`.
  pub fn serves(&self, model: &str) -> bool {
    self.models.iter().any(|listed| listed.id == model)
  }

  /// Whether a profile that ran on `before` and now runs on this sends the project's work somewhere else: to an
  /// API's host it did not go to, or to another account of the person's. That is a person's decision
  /// (`docs/design/agent-accounts.md`). A CLI's Default account is not such a move, nor is any choice for a profile
  /// that ran on nothing (`before` is `None`): no account is left for the work to move away from.
  pub fn moves_work(&self, before: Option<&Self>) -> bool {
    let Some(before) = before else {
      return false;
    };
    if self.host.is_some() {
      return self.host != before.host;
    }
    let default = AccountCli::of(self.cli).map(AccountCli::default_account);
    self.account != before.account && self.account != default
  }
}

/// What a profile can run on among `connections`, the oldest first: where an account has several, the first is the
/// one a choice of the account runs on. The connections `keeps` says list `own` too, the model the profile has, though
/// they do not list it themselves.
pub fn choices(
  connections: &[AgentConnection],
  own: Option<&ModelId>,
  keeps: impl Fn(&AgentConnection) -> bool,
) -> Vec<RunsOnChoice> {
  let mut agents: Vec<&AgentConnection> = connections
    .iter()
    .filter(|connection| connection.provider().runs_agents())
    .collect();
  agents.sort_by_key(|connection| connection.created_at());
  agents
    .into_iter()
    .map(|connection| {
      let own = own.filter(|_| keeps(connection));
      let models = model_options(connection, own)
        .into_iter()
        .map(|option| ModelChoice {
          efforts: effort_options(connection, &option.id),
          id: option.id.as_str().to_owned(),
        })
        .collect();
      let host = connection
        .api_provider()
        .map(|provider| provider.base_url().host().to_owned())
        .or_else(|| connection.api().map(|api| api.base_url().host().to_owned()));
      RunsOnChoice {
        id: connection.id(),
        name: connection.name().to_string(),
        cli: connection.provider(),
        account: connection.plan_account(),
        host,
        api_account: connection
          .api_provider()
          .map(|provider| ApiAccountId::from_uuid(provider.key().as_uuid())),
        signed_out: connection.status() == ConnectionStatus::SignInRequired,
        models,
      }
    })
    .collect()
}
