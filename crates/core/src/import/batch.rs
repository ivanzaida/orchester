//! What an import writes (06.06 `hF664`): new resources with their references resolved in the
//! target, where each came from, and the target's own resources, connections and secrets they
//! rely on.

use std::collections::{HashMap, HashSet};

use thiserror::Error;

use super::{
  Entity,
  catalog::{Catalog, Content, Pick},
  plan::{Plan, Resolution},
  references::Remap,
};
use crate::{
  id::{IdempotencyKey, ProjectId, ResourceId},
  name::Name,
  resource::{Resource, ResourceKind, Slug, VersionNumber},
  time::Timestamp,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ImportError {
  /// The plan copies nothing or has a problem left (see [`Plan::is_ready`]).
  #[error("the import is not ready")]
  NotReady,
  /// A copy with its references resolved breaks a rule of its kind.
  #[error("the copy of resource {resource} is not a valid {kind:?}: {detail}")]
  InvalidCopy {
    resource: ResourceId,
    kind: ResourceKind,
    detail: String,
  },
}

/// Where a copy came from, for display only: the copy does not follow its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
  pub project: ProjectId,
  pub project_name: Name,
  pub resource: ResourceId,
  /// The published version copied; `None` for a draft.
  pub version: Option<VersionNumber>,
}

/// A new resource of the target whose first draft is `content`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopiedResource {
  resource: Resource,
  content: Content,
  provenance: Provenance,
}

impl CopiedResource {
  pub const fn resource(&self) -> &Resource {
    &self.resource
  }

  pub const fn content(&self) -> &Content {
    &self.content
  }

  pub const fn provenance(&self) -> &Provenance {
    &self.provenance
  }
}

/// Everything one import creates in the target, written at once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportBatch {
  key: IdempotencyKey,
  copies: Vec<CopiedResource>,
  relies_on: Vec<Entity>,
}

impl ImportBatch {
  /// Identifies the import: writing a batch with a key already written creates nothing new.
  pub const fn key(&self) -> IdempotencyKey {
    self.key
  }

  /// The copies in plan order, selected resources first.
  pub fn copies(&self) -> &[CopiedResource] {
    &self.copies
  }

  /// The target's resources, connections and secrets the copies refer to, which must still exist
  /// when the batch is written.
  pub fn relies_on(&self) -> &[Entity] {
    &self.relies_on
  }
}

/// A copy the plan names, with the id it gets.
struct Named<'a> {
  source: ResourceId,
  pick: Pick,
  name: &'a Name,
  slug: &'a Slug,
  id: ResourceId,
}

impl Plan {
  /// The batch that carries out the plan as `key`, created at `now`. Every copy gets a new id;
  /// its references to copied resources, to the target's resources and to mapped connections,
  /// MCP servers and secrets are replaced, and unresolved ones stay as they are.
  pub fn batch(&self, source: &Catalog, key: IdempotencyKey, now: Timestamp) -> Result<ImportBatch, ImportError> {
    if !self.is_ready() {
      return Err(ImportError::NotReady);
    }
    let named: Vec<Named<'_>> = self
      .items()
      .iter()
      .filter_map(|item| {
        let (Entity::Resource(id), Some(pick)) = (item.source(), item.pick()) else {
          return None;
        };
        let (Resolution::Copy { name, slug } | Resolution::Rename(Some((name, slug)))) = item.resolution() else {
          return None;
        };
        Some(Named {
          source: id,
          pick,
          name,
          slug,
          id: ResourceId::generate(),
        })
      })
      .collect();
    let remap = self.remap(source, &named);
    let copies = named
      .iter()
      .map(|named| copy(source, &remap, named, now))
      .collect::<Result<_, _>>()?;
    let relies_on = self
      .items()
      .iter()
      .filter_map(|item| match item.resolution() {
        Resolution::Use(target) => Some(*target),
        _ => None,
      })
      .collect();
    Ok(ImportBatch { key, copies, relies_on })
  }

  fn remap(&self, source: &Catalog, named: &[Named<'_>]) -> Remap {
    let keys: HashMap<_, _> = named
      .iter()
      .map(|named| (named.source.as_uuid(), named.id.as_uuid()))
      .collect();
    let mut values = keys.clone();
    for item in self.items() {
      if let Resolution::Use(target) = item.resolution() {
        values.insert(item.source().uuid(), target.uuid());
      }
    }
    let known: HashSet<_> = source
      .resources()
      .iter()
      .map(|resource| resource.resource().id().as_uuid())
      .chain(source.connections().iter().map(|connection| connection.id().as_uuid()))
      .chain(source.mcp_servers().iter().map(|server| server.id().as_uuid()))
      .chain(source.secrets().iter().map(|secret| secret.id().as_uuid()))
      .collect();
    Remap { values, keys, known }
  }
}

fn copy(source: &Catalog, remap: &Remap, named: &Named<'_>, now: Timestamp) -> Result<CopiedResource, ImportError> {
  let content = source
    .resource(named.source)
    .and_then(|entry| entry.content(named.pick))
    .ok_or(ImportError::NotReady)?;
  let kind = content.kind();
  let content =
    Content::from_value(kind, remap.rewrite(content.to_value())).map_err(|error| ImportError::InvalidCopy {
      resource: named.source,
      kind,
      detail: error.to_string(),
    })?;
  let provenance = Provenance {
    project: source.project().id(),
    project_name: source.project().name().clone(),
    resource: named.source,
    version: match named.pick {
      Pick::Published(number) => Some(number),
      Pick::Draft => None,
    },
  };
  Ok(CopiedResource {
    resource: Resource::new(named.id, kind, named.slug.clone(), named.name.clone(), now),
    content,
    provenance,
  })
}
