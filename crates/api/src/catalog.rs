//! What a shared project's web editor can pick (`docs/architecture/server.md`, "Shared configuration", "What the web
//! editor can pick", row 75): the connections and MCP servers its members' desktops report, by kind, name and role,
//! and the model ids per connection role. Every shape refuses fields it does not know, so none can carry an account,
//! a path, a setting or a key besides the names, roles and model ids.

use std::collections::BTreeMap;

use orchester_core::resource::ModelId;
use serde::{Deserialize, Serialize};

/// The kinds of reference the catalog lists. Secrets are never reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogKind {
  Connection,
  McpServer,
}

/// A connection or an MCP server as the catalog lists it: its name, the person's own label, and, for a connection, its
/// role, the connection's kind (`claude_code_cli`, `openai_api`, …), as a published version's reference carries them.
/// An MCP server has no role.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogReference {
  pub kind: CatalogKind,
  pub name: String,
  #[serde(default)]
  pub role: Option<String>,
}

/// The model ids per connection role, each role's sorted.
pub type ModelsPerRole = BTreeMap<String, Vec<ModelId>>;

/// `PUT …/catalog`: this computer's whole state, replacing its device's previous report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogReport {
  pub references: Vec<CatalogReference>,
  pub models: ModelsPerRole,
}

/// `GET …/references`: the union of the members' reports with the references the project's versions name, once per
/// kind, role and name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogReferences {
  pub references: Vec<CatalogReference>,
}

/// `GET …/models`: the union of the reported model ids per role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogModels {
  pub models: ModelsPerRole,
}
