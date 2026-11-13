//! API accounts (`docs/design/agent-accounts.md`, "Agents run on accounts"): a model API reached with a key, kept
//! app-wide beside the sign-ins and offered to every project. The window calls one an API provider. An agent CLI
//! runs on it instead of its own sign-in (`docs/design/api-providers.md`), or Orchester's own agent talks to it
//! (`docs/architecture/native-agent.md`). The key is in the OS keychain under the account's id
//! ([`crate::secret::VaultKey::api_account`]), never in a project secret and never here.

use thiserror::Error;

use super::AccountCli;
use crate::{
  connection::{AgentProvider, BaseUrl, ConnectionCheck, EndpointPreset, NativeModels, offers_provider_fallback},
  id::ApiAccountId,
  name::Name,
  resource::ModelId,
  time::Timestamp,
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ApiAccountError {
  /// Profiles on the account choose from its models, so a CLI's API account needs at least one.
  #[error("an API provider needs at least one model")]
  NoModels,
  /// Only Orchester's own agents talk to a model API themselves.
  #[error("{0:?} is an agent CLI, not a model API Orchester calls itself")]
  NotNative(AgentProvider),
  #[error("the preset is not offered on this model API")]
  PresetNotOffered,
  #[error("provider fallback is offered on the official Anthropic API only")]
  FallbackNotOffered,
  /// An account keeps the kind of agent it was added for: what runs on it was chosen for that kind.
  #[error("an API provider keeps the kind of agent it was added for")]
  AnotherAgent,
  /// A CLI's API account has no check of its own: the CLI's program is what gets checked.
  #[error("only a model API Orchester calls itself is checked with its key")]
  CheckNotTaken,
}

/// What reaches the model API, and how.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiSetup {
  /// An agent CLI on a third-party API: Claude Code on an Anthropic Messages API, Codex on an OpenAI Responses API.
  /// Its models are typed by the person, kept once each in their order.
  Cli {
    cli: AccountCli,
    base_url: BaseUrl,
    models: Vec<ModelId>,
  },
  /// Orchester's own agent on a model API.
  Native {
    provider: AgentProvider,
    preset: EndpointPreset,
    base_url: BaseUrl,
    models: NativeModels,
    provider_fallback: bool,
  },
}

impl ApiSetup {
  /// A CLI's API account. A model named twice is kept once; no model at all is refused.
  pub fn cli(cli: AccountCli, base_url: BaseUrl, models: Vec<ModelId>) -> Result<Self, ApiAccountError> {
    let mut kept: Vec<ModelId> = Vec::with_capacity(models.len());
    for model in models {
      if !kept.contains(&model) {
        kept.push(model);
      }
    }
    if kept.is_empty() {
      return Err(ApiAccountError::NoModels);
    }
    Ok(Self::Cli {
      cli,
      base_url,
      models: kept,
    })
  }

  /// A model API Orchester's own agent of `provider` talks to. The preset must be one the provider offers, and
  /// provider fallback is on only where the provider offers it.
  pub fn native(
    provider: AgentProvider,
    preset: EndpointPreset,
    base_url: BaseUrl,
    models: NativeModels,
    provider_fallback: bool,
  ) -> Result<Self, ApiAccountError> {
    if !provider.is_native() {
      return Err(ApiAccountError::NotNative(provider));
    }
    if !preset.offered_on(provider) {
      return Err(ApiAccountError::PresetNotOffered);
    }
    if provider_fallback && !offers_provider_fallback(provider, preset) {
      return Err(ApiAccountError::FallbackNotOffered);
    }
    Ok(Self::Native {
      provider,
      preset,
      base_url,
      models,
      provider_fallback,
    })
  }

  /// The kind of agent that runs on the account.
  pub const fn agent(&self) -> AgentProvider {
    match self {
      Self::Cli { cli, .. } => cli.provider(),
      Self::Native { provider, .. } => *provider,
    }
  }

  pub const fn base_url(&self) -> &BaseUrl {
    match self {
      Self::Cli { base_url, .. } | Self::Native { base_url, .. } => base_url,
    }
  }

  /// Whether `other` reaches the same API as the same kind of agent: a check of one holds for the other, whose
  /// models and provider fallback may differ.
  fn same_api(&self, other: &Self) -> bool {
    let preset = |setup: &Self| match setup {
      Self::Cli { .. } => None,
      Self::Native { preset, .. } => Some(*preset),
    };
    self.agent() == other.agent() && self.base_url() == other.base_url() && preset(self) == preset(other)
  }
}

/// One API account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiAccount {
  id: ApiAccountId,
  name: Name,
  setup: ApiSetup,
  added_at: Timestamp,
  last_check: Option<ConnectionCheck>,
}

impl ApiAccount {
  /// A new account, not checked yet.
  pub const fn new(id: ApiAccountId, name: Name, setup: ApiSetup, added_at: Timestamp) -> Self {
    Self {
      id,
      name,
      setup,
      added_at,
      last_check: None,
    }
  }

  /// An account as stored, with the check made of it.
  pub fn from_stored(
    id: ApiAccountId,
    name: Name,
    setup: ApiSetup,
    added_at: Timestamp,
    last_check: Option<ConnectionCheck>,
  ) -> Result<Self, ApiAccountError> {
    let mut account = Self::new(id, name, setup, added_at);
    if let Some(check) = last_check {
      account.record_check(check)?;
    }
    Ok(account)
  }

  pub const fn id(&self) -> ApiAccountId {
    self.id
  }

  pub const fn name(&self) -> &Name {
    &self.name
  }

  pub const fn setup(&self) -> &ApiSetup {
    &self.setup
  }

  /// The kind of agent that runs on the account.
  pub const fn agent(&self) -> AgentProvider {
    self.setup.agent()
  }

  pub const fn added_at(&self) -> Timestamp {
    self.added_at
  }

  pub const fn last_check(&self) -> Option<&ConnectionCheck> {
    self.last_check.as_ref()
  }

  /// Changes only the name people see: what runs on the account holds its id.
  pub fn rename(&mut self, name: Name) {
    self.name = name;
  }

  /// Points the account at another setup for the same kind of agent. The last check stays while the API does;
  /// another base URL or preset drops it.
  pub fn set_setup(&mut self, setup: ApiSetup) -> Result<(), ApiAccountError> {
    if setup.agent() != self.setup.agent() {
      return Err(ApiAccountError::AnotherAgent);
    }
    if !self.setup.same_api(&setup) {
      self.last_check = None;
    }
    self.setup = setup;
    Ok(())
  }

  /// Drops the last check: it was made with a key that has been replaced.
  pub fn key_replaced(&mut self) {
    self.last_check = None;
  }

  /// Takes in what testing the model API with the account's key found. Only a model API Orchester calls itself is
  /// tested so, and only a check of an API describes it.
  pub fn record_check(&mut self, check: ConnectionCheck) -> Result<(), ApiAccountError> {
    if !matches!(self.setup, ApiSetup::Native { .. }) || !check.result.is_api() {
      return Err(ApiAccountError::CheckNotTaken);
    }
    self.last_check = Some(check);
    Ok(())
  }
}
