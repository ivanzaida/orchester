//! What a shared project's web editor can pick (`docs/architecture/server.md`, "Shared configuration", "What the web
//! editor can pick", row 75): the CLI kinds its members' desktops report and the model ids per kind. A CLI kind and a
//! model role are the four agent connection roles, as core's [`ConnectionRole::as_str`] writes them. Every shape
//! refuses fields it does not know, so none can carry a connection or MCP server name, an account, a path, a setting
//! or a key.

use std::collections::BTreeMap;

use orchester_core::{
  resource::ModelId,
  sharing::{CATALOG_MODELS_PER_ROLE, ConnectionRole},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The most CLI kinds one report may carry: the agent roles there are.
pub const CATALOG_CLI_KINDS: usize = 4;

/// The model ids per connection role, each role's sorted.
pub type ModelsPerRole = BTreeMap<String, Vec<ModelId>>;

/// `PUT …/catalog`: this computer's whole state, replacing its device's previous report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogReport {
  pub cli_kinds: Vec<String>,
  pub models: ModelsPerRole,
}

/// Why a report is refused (`422 invalid { reason: "catalog" }`).
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CatalogRefused {
  #[error("{0:?} is not an agent role")]
  NotAnAgentRole(String),
  #[error("a report names at most {CATALOG_CLI_KINDS} CLI kinds")]
  TooManyCliKinds,
  #[error("a report names at most {CATALOG_MODELS_PER_ROLE} model ids per role")]
  TooManyModels,
}

impl CatalogReport {
  /// Whether the server takes this report: each CLI kind and each model role is an agent role (`github` and a role
  /// this version does not know are not), at most [`CATALOG_CLI_KINDS`] CLI kinds and [`CATALOG_MODELS_PER_ROLE`] model
  /// ids per role.
  pub fn check(&self) -> Result<(), CatalogRefused> {
    if self.cli_kinds.len() > CATALOG_CLI_KINDS {
      return Err(CatalogRefused::TooManyCliKinds);
    }
    for role in self.cli_kinds.iter().chain(self.models.keys()) {
      if !matches!(ConnectionRole::parse(role), Some(ConnectionRole::Agent(_))) {
        return Err(CatalogRefused::NotAnAgentRole(role.clone()));
      }
    }
    if self.models.values().any(|ids| ids.len() > CATALOG_MODELS_PER_ROLE) {
      return Err(CatalogRefused::TooManyModels);
    }
    Ok(())
  }
}

/// `GET …/cli-kinds`: the union of the members' reports, sorted, once each.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogCliKinds {
  pub cli_kinds: Vec<String>,
}

/// `GET …/models`: the union of the reported model ids per role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogModels {
  pub models: ModelsPerRole,
}
