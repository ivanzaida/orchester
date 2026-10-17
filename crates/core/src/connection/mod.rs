//! Project connections (`docs/design/orchester-workspace.md`, "Agent profiles and connections").
//! Two kinds are programs on this computer: agent providers (the Codex and Claude Code CLIs) and
//! GitHub through the GitHub CLI. MCP servers ([`mcp`]) are the third kind, configured for the agent
//! CLIs to start or reach. A program connection is signed in through the program's own login; Orchester
//! stores no credentials and never asks the programs for them. It holds the executable and what the last
//! check found out about it. A native agent provider ([`AgentProvider::is_native`]) is reached at a model
//! API instead ([`AgentEndpoint::Api`]), with its key in a project secret; this version stores and shows
//! such connections but runs no agent on them. An agent CLI connection can instead run on an API provider
//! ([`ApiProvider`]): a third-party model API reached with a key in a project secret.

mod account;
mod adapter;
mod agent;
mod api_provider;
pub mod app_server;
pub mod app_server_config;
pub mod app_server_mcp;
mod catalog;
pub mod claude_approval;
mod claude_catalog;
mod discovery;
mod endpoint;
mod github;
mod http_url;
pub mod invocation;
pub mod mcp;
mod model_requirements;
mod native_models;
pub mod plan_limits;
mod provider;
pub mod reviewer;
pub mod stream;

use std::path::{Path, PathBuf};

use thiserror::Error;

pub(crate) use self::github::gh_command;
pub use self::{
  account::{AccountStartProblem, ConnectionAccount, moves_to_another_account},
  adapter::{
    check, executable_candidates, parse_claude_auth_status, parse_codex_catalog, parse_codex_login_status,
    parse_version,
  },
  agent::{AgentConnection, CheckResult, ConnectionCheck, EngineVendor},
  api_provider::{ApiProvider, ApiProviderError},
  catalog::{CatalogModel, CatalogSource, ModelCatalog, ModelSupport, REASONING_EFFORT},
  claude_catalog::parse_claude_models,
  discovery::{Installed, compare_versions, rank_installed},
  endpoint::{AgentEndpoint, ApiEndpoint, BaseUrl, EndpointError, EndpointPreset, offers_provider_fallback},
  github::{
    GH_PROGRAM, GhAdvice, GhAuthStatus, GitHubCheck, GitHubCheckResult, GitHubConnection, GitHubHost, GitHubLogin,
    GitHubNameError, GitHubState, REQUIRED_SCOPE, check_github, gh_auth_status_command, gh_version_command,
    parse_gh_auth_status, parse_gh_version,
  },
  model_requirements::{CliTooOld, ModelRequirements, parse_cli_too_old},
  native_models::{Effort, ModelSource, NativeModel, NativeModels, NativeModelsError, ensure_used_kept, with_fetched},
  provider::{AgentProvider, AgentRuntime, WireApi},
};
use crate::{
  id::{ConnectionId, SecretId},
  name::Name,
  time::Timestamp,
};

/// What a connection connects to.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionKind {
  /// An agent CLI that runs the steps of agent profiles.
  Agent(AgentProvider),
  /// The GitHub CLI, signed in on `host`.
  GitHub { host: GitHubHost },
}

impl ConnectionKind {
  /// Name of the program on the `PATH`; `None` for a native agent provider.
  pub const fn program_name(&self) -> Option<&'static str> {
    match self {
      Self::Agent(provider) => provider.program_name(),
      Self::GitHub { .. } => Some(GH_PROGRAM),
    }
  }

  /// Whether the connection is reached at a model API rather than a program.
  const fn is_api(&self) -> bool {
    matches!(self, Self::Agent(provider) if provider.is_native())
  }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConnectionError {
  #[error("the executable path `{0}` is not absolute")]
  RelativeExecutable(PathBuf),
  #[error("connection {0} is not an agent provider connection")]
  NotAgent(ConnectionId),
  #[error("connection {0} is not a GitHub connection")]
  NotGitHub(ConnectionId),
  #[error("the check does not belong to the kind of connection {0}")]
  CheckKindMismatch(ConnectionId),
  /// A program for a native provider, or an API endpoint for a program.
  #[error("connection {0} is not reached that way")]
  EndpointMismatch(ConnectionId),
  #[error("the DeepSeek preset is not offered on the Anthropic API")]
  PresetNotOffered,
  #[error("provider fallback is offered on the official Anthropic API only")]
  FallbackNotOffered,
  /// Only an agent CLI connection runs on an account.
  #[error("connection {0} runs no agent CLI, so it has no account")]
  NoAccounts(ConnectionId),
  #[error("the account is not one of this connection's CLI")]
  AccountOfOtherCli,
  /// Only an agent CLI connection runs on an API provider (`docs/design/api-providers.md`).
  #[error("connection {0} runs no agent CLI, so it runs on no API provider")]
  NoApiProvider(ConnectionId),
  /// A connection on an API provider runs on its key, not on a sign-in of its CLI.
  #[error("a connection on an API provider runs on no account of its CLI")]
  ApiProviderWithAccount,
}

/// Whether the CLI is signed in, as far as its own status command tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignIn {
  SignedIn,
  SignedOut,
  /// The CLI has no status command, or its answer could not be read.
  Unknown,
}

/// State of a connection, derived from its last check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
  NotChecked,
  Connected,
  SignInRequired,
  /// The CLI works, but whether it is signed in could not be determined.
  SignInUnknown,
  ExecutableMissing,
  NotRecognized,
  CouldNotRun,
  /// A native connection's key secret is archived or gone.
  KeyMissing,
  /// The model API refused the key (401).
  KeyRejected,
  /// The model API accepted the key but refuses to serve it for billing (402).
  BillingProblem,
  /// The model API could not be reached or did not answer in time.
  EndpointUnreachable,
}

impl ConnectionStatus {
  /// Stored alongside the check for fast reads.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::NotChecked => "not_checked",
      Self::Connected => "connected",
      Self::SignInRequired => "sign_in_required",
      Self::SignInUnknown => "sign_in_unknown",
      Self::ExecutableMissing => "executable_missing",
      Self::NotRecognized => "not_recognized",
      Self::CouldNotRun => "could_not_run",
      Self::KeyMissing => "key_missing",
      Self::KeyRejected => "key_rejected",
      Self::BillingProblem => "billing_problem",
      Self::EndpointUnreachable => "endpoint_unreachable",
    }
  }

  /// The program can be used through the connection, or at least nothing says it cannot.
  pub const fn is_usable(self) -> bool {
    matches!(self, Self::Connected | Self::SignInUnknown)
  }
}

/// The last check of a connection, of the connection's kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LastCheck {
  Agent(ConnectionCheck),
  GitHub(GitHubCheck),
}

impl LastCheck {
  pub const fn at(&self) -> Timestamp {
    match self {
      Self::Agent(check) => check.at,
      Self::GitHub(check) => check.at,
    }
  }

  pub const fn status(&self) -> ConnectionStatus {
    match self {
      Self::Agent(check) => check.result.status(),
      Self::GitHub(check) => check.result.status(),
    }
  }

  /// Version of the program, when the check found the expected one.
  pub fn version(&self) -> Option<&str> {
    match self {
      Self::Agent(ConnectionCheck {
        result: CheckResult::Available { version, .. },
        ..
      })
      | Self::GitHub(GitHubCheck {
        result: GitHubCheckResult::Available { version, .. },
        ..
      }) => Some(version),
      Self::Agent(_) | Self::GitHub(_) => None,
    }
  }

  /// Whether the check can describe a connection of `kind`: a check of a model API describes a native
  /// agent connection, a check of a program any other.
  const fn fits(&self, kind: &ConnectionKind) -> bool {
    match (self, kind) {
      (Self::Agent(check), ConnectionKind::Agent(provider)) => check.result.is_api() == provider.is_native(),
      (Self::GitHub(_), ConnectionKind::GitHub { .. }) => true,
      (Self::Agent(_), ConnectionKind::GitHub { .. }) | (Self::GitHub(_), ConnectionKind::Agent(_)) => false,
    }
  }
}

/// A project connection. [`AgentConnection`] and [`GitHubConnection`] are the typed views that
/// code needing one kind works with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
  id: ConnectionId,
  name: Name,
  kind: ConnectionKind,
  endpoint: AgentEndpoint,
  last_check: Option<LastCheck>,
  created_at: Timestamp,
  /// The CLI versions models need, as refusals said ([`ModelRequirements`]); agent CLIs only.
  model_requirements: ModelRequirements,
  /// Whose sign-in an agent CLI connection runs on; Default for every other kind.
  account: ConnectionAccount,
  /// The model API an agent CLI connection runs on instead of its CLI's own sign-in, if any.
  api_provider: Option<ApiProvider>,
}

impl Connection {
  /// A connection that has not been checked yet.
  pub fn new(
    id: ConnectionId,
    name: Name,
    kind: ConnectionKind,
    endpoint: AgentEndpoint,
    created_at: Timestamp,
  ) -> Result<Self, ConnectionError> {
    let endpoint = fitted(id, &kind, endpoint)?;
    Ok(Self {
      id,
      name,
      kind,
      endpoint,
      last_check: None,
      created_at,
      model_requirements: ModelRequirements::default(),
      account: ConnectionAccount::Default,
      api_provider: None,
    })
  }

  /// This connection with the model requirements it had recorded.
  #[must_use]
  pub fn with_model_requirements(mut self, requirements: ModelRequirements) -> Self {
    self.model_requirements = requirements;
    self
  }

  pub const fn model_requirements(&self) -> &ModelRequirements {
    &self.model_requirements
  }

  /// This connection on the account it had recorded, with the check made on it.
  pub fn with_account(mut self, account: ConnectionAccount) -> Result<Self, ConnectionError> {
    let check = self.last_check.take();
    self.set_account(account)?;
    self.last_check = check;
    Ok(self)
  }

  pub const fn account(&self) -> &ConnectionAccount {
    &self.account
  }

  /// Runs the connection's CLI on `account`. Another account drops the last check: it read the sign-in of the
  /// previous one. Only an agent CLI connection has an account other than Default.
  pub fn set_account(&mut self, account: ConnectionAccount) -> Result<(), ConnectionError> {
    if account == self.account {
      return Ok(());
    }
    if self.api_provider.is_some() && !account.is_default() {
      return Err(ConnectionError::ApiProviderWithAccount);
    }
    let ConnectionKind::Agent(provider) = self.kind else {
      return Err(ConnectionError::NoAccounts(self.id));
    };
    if crate::account::AccountCli::of(provider).is_none() {
      return Err(ConnectionError::NoAccounts(self.id));
    }
    self.account = account;
    self.last_check = None;
    Ok(())
  }

  /// Runs the connection's CLI on `account`, which must be an account of that CLI.
  pub fn choose_account(&mut self, account: &crate::account::AgentAccount) -> Result<(), ConnectionError> {
    if self.kind != ConnectionKind::Agent(account.cli().provider()) {
      return Err(ConnectionError::AccountOfOtherCli);
    }
    self.set_account(ConnectionAccount::of(account))
  }

  /// This connection on the API provider it had recorded, with the check made on it.
  pub fn with_api_provider(mut self, provider: Option<ApiProvider>) -> Result<Self, ConnectionError> {
    self.set_api_provider(provider)?;
    Ok(self)
  }

  /// The model API the connection's CLI runs on instead of its own sign-in.
  pub const fn api_provider(&self) -> Option<&ApiProvider> {
    self.api_provider.as_ref()
  }

  /// The project secret holding the key of the connection's model API: a native connection's, or an API
  /// provider's.
  pub fn key(&self) -> Option<SecretId> {
    self
      .endpoint
      .api()
      .map(ApiEndpoint::key)
      .or_else(|| self.api_provider().map(ApiProvider::key))
  }

  /// Runs the connection's CLI on `provider`, or on its own sign-in again (`None`). Only an agent CLI connection
  /// runs on one, and only on its CLI's Default account. The last check stays: it read the program, which does not
  /// change.
  pub fn set_api_provider(&mut self, provider: Option<ApiProvider>) -> Result<(), ConnectionError> {
    if provider.is_some() {
      let ConnectionKind::Agent(agent) = self.kind else {
        return Err(ConnectionError::NoApiProvider(self.id));
      };
      if agent.provider_wire().is_none() {
        return Err(ConnectionError::NoApiProvider(self.id));
      }
      if !self.account.is_default() {
        return Err(ConnectionError::ApiProviderWithAccount);
      }
    }
    self.api_provider = provider;
    Ok(())
  }

  pub fn from_stored(
    id: ConnectionId,
    name: Name,
    kind: ConnectionKind,
    endpoint: AgentEndpoint,
    last_check: Option<LastCheck>,
    created_at: Timestamp,
  ) -> Result<Self, ConnectionError> {
    let mut connection = Self::new(id, name, kind, endpoint, created_at)?;
    if let Some(check) = last_check {
      connection.record_check(check)?;
    }
    Ok(connection)
  }

  pub const fn id(&self) -> ConnectionId {
    self.id
  }

  pub fn name(&self) -> &Name {
    &self.name
  }

  pub const fn kind(&self) -> &ConnectionKind {
    &self.kind
  }

  pub const fn endpoint(&self) -> &AgentEndpoint {
    &self.endpoint
  }

  /// The program the connection runs; `None` when it is reached at a model API.
  pub fn executable(&self) -> Option<&Path> {
    self.endpoint.program()
  }

  pub const fn last_check(&self) -> Option<&LastCheck> {
    self.last_check.as_ref()
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }

  pub fn rename(&mut self, name: Name) {
    self.name = name;
  }

  /// Points the connection at another program. The last check described the previous one,
  /// so it is dropped. A connection reached at a model API has no program to change.
  pub fn set_executable(&mut self, executable: PathBuf) -> Result<(), ConnectionError> {
    let endpoint = fitted(self.id, &self.kind, AgentEndpoint::Program(executable))?;
    if endpoint != self.endpoint {
      self.endpoint = endpoint;
      self.last_check = None;
    }
    Ok(())
  }

  /// Points a native connection at another model API setup. The last check stays while the API and
  /// its key do; another preset, base URL or key drops it.
  pub fn set_api(&mut self, api: ApiEndpoint) -> Result<(), ConnectionError> {
    let same_api = self.endpoint.api().is_some_and(|current| {
      current.preset() == api.preset() && current.base_url() == api.base_url() && current.key() == api.key()
    });
    self.endpoint = fitted(self.id, &self.kind, AgentEndpoint::Api(api))?;
    if !same_api {
      self.last_check = None;
    }
    Ok(())
  }

  /// Replaces the last check. A check of another kind of connection, or of a program for a native
  /// agent connection (or of a model API for any other), is refused.
  pub fn record_check(&mut self, check: LastCheck) -> Result<(), ConnectionError> {
    if !check.fits(&self.kind) {
      return Err(ConnectionError::CheckKindMismatch(self.id));
    }
    self.last_check = Some(check);
    Ok(())
  }

  /// The last check's status. A connection on an API provider does not use its CLI's own sign-in, so once the
  /// check found the CLI it reads Connected whatever the sign-in was.
  pub fn status(&self) -> ConnectionStatus {
    match &self.last_check {
      None => ConnectionStatus::NotChecked,
      Some(LastCheck::Agent(ConnectionCheck {
        result: CheckResult::Available { .. },
        ..
      })) if self.api_provider.is_some() => ConnectionStatus::Connected,
      Some(check) => check.status(),
    }
  }

  /// Program version found by the last successful check.
  pub fn version(&self) -> Option<&str> {
    self.last_check.as_ref()?.version()
  }
}

/// `endpoint` if it is how a connection of `kind` is reached: an absolute program path for a CLI,
/// an API endpoint with a preset (and provider fallback) its provider offers for a native agent.
fn fitted(id: ConnectionId, kind: &ConnectionKind, endpoint: AgentEndpoint) -> Result<AgentEndpoint, ConnectionError> {
  match (&endpoint, kind) {
    (AgentEndpoint::Program(path), _) if !kind.is_api() && !path.is_absolute() => {
      Err(ConnectionError::RelativeExecutable(path.clone()))
    }
    (AgentEndpoint::Api(api), ConnectionKind::Agent(provider)) if kind.is_api() => {
      if !api.preset().offered_on(*provider) {
        return Err(ConnectionError::PresetNotOffered);
      }
      if api.provider_fallback() && !offers_provider_fallback(*provider, api.preset()) {
        return Err(ConnectionError::FallbackNotOffered);
      }
      Ok(endpoint)
    }
    (AgentEndpoint::Program(_), _) if !kind.is_api() => Ok(endpoint),
    _ => Err(ConnectionError::EndpointMismatch(id)),
  }
}
