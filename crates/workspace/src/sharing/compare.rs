//! Each version of a local project's resource as *Versions* lists it (03 `pjEjf`), with what it changed from the one
//! before it, field by field: bodies are compared as their JSON. A team project's resources have no versions on this
//! computer: the team server keeps them (`docs/design/team-configuration-live.md`).

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{ResourceId, VersionId},
  resource::{AgentProfile, Instruction, Pipeline, ResourceBody, ResourceKind, Skill, Tool, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use serde_json::{Map, Value};

/// One version of a resource in *Versions*.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
  pub number: VersionNumber,
  pub published_at: Timestamp,
  /// The fields it changed from the version before it; every field of a first version.
  pub changed: Vec<Arc<str>>,
  /// The version runs here use.
  pub used_here: bool,
}

/// One version of a resource as it is kept here, its body as JSON.
struct Kept {
  id: VersionId,
  number: VersionNumber,
  published_at: Timestamp,
  body: Value,
}

/// The versions of `resource` (of `kind`) in a local project, newest first. Blocking.
pub fn local_history(
  repo_root: &Path,
  kind: ResourceKind,
  resource: ResourceId,
) -> Result<Vec<HistoryEntry>, StoreError> {
  let (kept, used) = kept(&ProjectDb::open(repo_root)?, kind, resource)?;
  let mut before = Value::Null;
  let mut entries = Vec::new();
  for version in kept {
    entries.push(HistoryEntry {
      number: version.number,
      published_at: version.published_at,
      changed: changed(&before, &version.body),
      used_here: used == Some(version.id),
    });
    before = version.body;
  }
  entries.reverse();
  Ok(entries)
}

/// The fields `to` holds differently from `from`, by their names in the body, in name order.
fn changed(from: &Value, to: &Value) -> Vec<Arc<str>> {
  let empty = Map::new();
  let (from, to) = (from.as_object().unwrap_or(&empty), to.as_object().unwrap_or(&empty));
  let mut names: Vec<&String> = from.keys().chain(to.keys()).collect();
  names.sort();
  names.dedup();
  names
    .into_iter()
    .filter(|name| from.get(*name) != to.get(*name))
    .map(|name| Arc::from(name.as_str()))
    .collect()
}

/// Every version of `resource` kept here, oldest first, and the one runs here use.
fn kept(
  db: &ProjectDb,
  kind: ResourceKind,
  resource: ResourceId,
) -> Result<(Vec<Kept>, Option<VersionId>), StoreError> {
  match kind {
    ResourceKind::AgentProfile => kept_of::<AgentProfile>(db, resource),
    ResourceKind::Pipeline => kept_of::<Pipeline>(db, resource),
    ResourceKind::Instruction => kept_of::<Instruction>(db, resource),
    ResourceKind::Skill => kept_of::<Skill>(db, resource),
    ResourceKind::Tool => kept_of::<Tool>(db, resource),
    // A local project has no team reviewers or team MCP servers, and agent testing's kinds have no versions here.
    ResourceKind::TeamReviewer
    | ResourceKind::McpServer
    | ResourceKind::TestSuite
    | ResourceKind::Service
    | ResourceKind::Stack => Ok((Vec::new(), None)),
  }
}

fn kept_of<B: ResourceBody>(
  db: &ProjectDb,
  resource: ResourceId,
) -> Result<(Vec<Kept>, Option<VersionId>), StoreError> {
  let used = db.latest_version::<B>(resource)?.map(|version| version.id());
  let kept = db
    .versions::<B>(resource)?
    .into_iter()
    .map(|version| {
      let body = serde_json::to_value(version.body()).map_err(|error| StoreError::Corrupt {
        what: "resource body",
        detail: error.to_string(),
      })?;
      Ok(Kept {
        id: version.id(),
        number: version.number(),
        published_at: version.published_at(),
        body,
      })
    })
    .collect::<Result<_, StoreError>>()?;
  Ok((kept, used))
}
