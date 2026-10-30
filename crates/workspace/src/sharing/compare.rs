//! What versions of one of the team's resources hold differently, field by field (`Shared configuration · redesign`
//! `UC5xk`): what a newer version changed from the one a version published here was edited from, and that version
//! against the newer one, *Compare with vN* (02c `JZFlA`); and each version a member reads in *Versions*, with what it
//! changed from the one before it (03 `pjEjf`). Bodies are compared as their JSON, field by field; nothing is merged.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{ResourceId, VersionId},
  resource::{AgentProfile, Instruction, Pipeline, ResourceBody, ResourceKind, Skill, Tool, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use serde_json::{Map, Value};

/// One field of a resource's body that two versions hold differently: its name in the body (`instructions`) and each
/// version's value as text, empty where that version has none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldChange {
  pub field: Arc<str>,
  pub from: Arc<str>,
  pub to: Arc<str>,
}

/// What a version published here that met a newer one shows: what the newer one changed from the version both were
/// edited from, and the fields the one published here holds differently from the newer one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictComparison {
  pub latest_changed: Vec<Arc<str>>,
  pub against_latest: Vec<FieldChange>,
}

/// One version of a resource in *Versions*, as the server numbered it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
  pub number: VersionNumber,
  pub published_at: Timestamp,
  /// Who published it, by their name on the server; `None` for a version published here before the project was shared.
  pub publisher: Option<String>,
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

/// The fields `to` holds differently from `from`, by their names in the body, in name order.
pub fn changes(from: &Value, to: &Value) -> Vec<FieldChange> {
  let empty = Map::new();
  let (from, to) = (from.as_object().unwrap_or(&empty), to.as_object().unwrap_or(&empty));
  let mut names: Vec<&String> = from.keys().chain(to.keys()).collect();
  names.sort();
  names.dedup();
  names
    .into_iter()
    .filter_map(|name| {
      let (before, after) = (from.get(name), to.get(name));
      (before != after).then(|| FieldChange {
        field: Arc::from(name.as_str()),
        from: text(before),
        to: text(after),
      })
    })
    .collect()
}

/// *Compare with vN* and what vN changed, for `mine`, a version of `resource` (of `kind`) published here and edited
/// from `base`, which met `latest`; `None` while `latest` is not kept here (a teammate's skill is not). Blocking.
pub fn conflict(
  repo_root: &Path,
  (kind, resource): (ResourceKind, ResourceId),
  mine: VersionId,
  (base, latest): (Option<VersionNumber>, VersionNumber),
) -> Result<Option<ConflictComparison>, StoreError> {
  let (kept, _) = kept(&ProjectDb::open(repo_root)?, kind, resource)?;
  let by_number = |number| kept.iter().find(|version| version.number == number);
  let (Some(newer), Some(mine)) = (by_number(latest), kept.iter().find(|version| version.id == mine)) else {
    return Ok(None);
  };
  let base = base
    .and_then(by_number)
    .map_or(Value::Null, |version| version.body.clone());
  Ok(Some(ConflictComparison {
    latest_changed: changes(&base, &newer.body)
      .into_iter()
      .map(|change| change.field)
      .collect(),
    against_latest: changes(&newer.body, &mine.body),
  }))
}

/// The versions of `resource` (of `kind`) the server numbered, newest first, each with who published it and what it
/// changed. Blocking.
pub fn history(repo_root: &Path, kind: ResourceKind, resource: ResourceId) -> Result<Vec<HistoryEntry>, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  let publishers = db.version_publishers(resource)?;
  let (kept, used) = kept(&db, kind, resource)?;
  let mut before = Value::Null;
  let mut entries = Vec::new();
  for version in kept {
    let Some((_, publisher)) = publishers.iter().find(|(id, _)| *id == version.id) else {
      // A version published here that waits for the server has no number there yet.
      continue;
    };
    entries.push(HistoryEntry {
      number: version.number,
      published_at: version.published_at,
      publisher: publisher.clone(),
      changed: changes(&before, &version.body)
        .into_iter()
        .map(|change| change.field)
        .collect(),
      used_here: used == Some(version.id),
    });
    before = version.body;
  }
  entries.reverse();
  Ok(entries)
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
    // Agent testing's kinds are never the team's.
    ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack => Ok((Vec::new(), None)),
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

/// A field's value as text: a string as it is, anything else as indented JSON; empty for none.
fn text(value: Option<&Value>) -> Arc<str> {
  match value {
    None | Some(Value::Null) => Arc::from(""),
    Some(Value::String(text)) => Arc::from(text.as_str()),
    // A JSON value always serializes; its one-line form is the same value should indenting it ever fail.
    Some(value) => Arc::from(serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())),
  }
}
