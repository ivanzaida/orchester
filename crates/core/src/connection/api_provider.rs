//! A third-party model API an agent CLI runs on with an API key instead of the CLI's own sign-in
//! (`docs/design/api-providers.md`): Claude Code on an Anthropic Messages API, Codex on an OpenAI Responses API.
//! Like a native connection's endpoint it names the project secret holding the key, never the key itself.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{AgentConnection, BaseUrl, catalog::CatalogModel};
use crate::{
  id::{AgentAccountId, SecretId},
  resource::ModelId,
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ApiProviderError {
  /// Profiles on the connection choose from its models, so it needs at least one.
  #[error("an API provider needs at least one model")]
  NoModels,
}

/// The model API, the project secret holding its key and the models it serves, which are the models agent
/// profiles on the connection choose from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "StoredApiProvider", into = "StoredApiProvider")]
pub struct ApiProvider {
  base_url: BaseUrl,
  key: SecretId,
  models: Vec<CatalogModel>,
}

impl ApiProvider {
  /// The models are kept once each, in the order given; none is refused.
  pub fn new(base_url: BaseUrl, key: SecretId, models: Vec<ModelId>) -> Result<Self, ApiProviderError> {
    let mut kept: Vec<ModelId> = Vec::with_capacity(models.len());
    for model in models {
      if !kept.contains(&model) {
        kept.push(model);
      }
    }
    if kept.is_empty() {
      return Err(ApiProviderError::NoModels);
    }
    let models = kept.into_iter().map(|id| CatalogModel::new(id, Vec::new())).collect();
    Ok(Self { base_url, key, models })
  }

  pub const fn base_url(&self) -> &BaseUrl {
    &self.base_url
  }

  /// The project secret holding the API key.
  pub const fn key(&self) -> SecretId {
    self.key
  }

  /// The models typed for the provider, in order; none offers a reasoning effort.
  pub fn models(&self) -> &[CatalogModel] {
    &self.models
  }

  /// The model ids, in order.
  pub fn model_ids(&self) -> impl Iterator<Item = &ModelId> {
    self.models.iter().map(CatalogModel::id)
  }
}

/// What an agent CLI's session was made on, or would be continued on now: the account whose folder keeps it, and the
/// API provider (its base URL and key secret) that served it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionOrigin {
  /// `None` where no account was recorded: a session from before accounts, or one made on an API provider.
  pub account: Option<AgentAccountId>,
  pub provider: Option<(BaseUrl, SecretId)>,
}

impl SessionOrigin {
  /// What a session started on `connection` now is made on.
  pub fn of(connection: &AgentConnection) -> Self {
    Self {
      account: connection.plan_account(),
      provider: connection
        .api_provider()
        .map(|provider| (provider.base_url().clone(), provider.key())),
    }
  }

  /// Whether a session made on `self` continues on `now`: not in another account's folder, which does not hold it,
  /// and not on another API provider, another host or key, nor on one where there was none or none where there was
  /// one, whose servers did not see its conversation.
  pub fn continues_on(&self, now: &Self) -> bool {
    let same_account = self.account.is_none() || self.account == now.account;
    same_account && self.provider == now.provider
  }
}

/// How an [`ApiProvider`] is stored: the models as their ids.
#[derive(Serialize, Deserialize)]
struct StoredApiProvider {
  base_url: BaseUrl,
  key: SecretId,
  models: Vec<ModelId>,
}

impl TryFrom<StoredApiProvider> for ApiProvider {
  type Error = ApiProviderError;

  fn try_from(stored: StoredApiProvider) -> Result<Self, ApiProviderError> {
    Self::new(stored.base_url, stored.key, stored.models)
  }
}

impl From<ApiProvider> for StoredApiProvider {
  fn from(provider: ApiProvider) -> Self {
    let models = provider.model_ids().cloned().collect();
    Self {
      base_url: provider.base_url,
      key: provider.key,
      models,
    }
  }
}
