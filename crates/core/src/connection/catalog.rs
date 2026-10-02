use serde::{Deserialize, Serialize};

use crate::resource::ModelId;

/// Provider setting holding the reasoning effort a model runs with. Its values come from the
/// adapter's catalog; a profile without it uses the CLI's default.
pub const REASONING_EFFORT: &str = "reasoning_effort";

/// Where the model list of a connection comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSource {
  /// The CLI listed its models.
  Provider,
  /// The CLI could not list models; these are the aliases its own help documents (Claude Code:
  /// after Anthropic's current model ids).
  DocumentedAliases,
  /// Claude Code listed the models it knows by alias; Anthropic's current model ids come first.
  ProviderAndCurrentIds,
}

/// A model the connection offers and the reasoning efforts it accepts (none: the setting is
/// not offered).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogModel {
  id: ModelId,
  efforts: Vec<String>,
}

impl CatalogModel {
  pub const fn new(id: ModelId, efforts: Vec<String>) -> Self {
    Self { id, efforts }
  }

  pub const fn id(&self) -> &ModelId {
    &self.id
  }

  pub fn efforts(&self) -> &[String] {
    &self.efforts
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCatalog {
  source: CatalogSource,
  models: Vec<CatalogModel>,
}

/// Whether a model id is known to work on a connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelSupport {
  Listed,
  /// Entered by hand or no longer listed: the CLI decides when an agent starts.
  NotVerified,
}

impl ModelCatalog {
  pub const fn new(source: CatalogSource, models: Vec<CatalogModel>) -> Self {
    Self { source, models }
  }

  pub const fn source(&self) -> CatalogSource {
    self.source
  }

  pub fn models(&self) -> &[CatalogModel] {
    &self.models
  }

  pub fn model(&self, id: &ModelId) -> Option<&CatalogModel> {
    self.models.iter().find(|model| model.id() == id)
  }

  pub fn support(&self, id: &ModelId) -> ModelSupport {
    if self.model(id).is_some() {
      ModelSupport::Listed
    } else {
      ModelSupport::NotVerified
    }
  }
}
