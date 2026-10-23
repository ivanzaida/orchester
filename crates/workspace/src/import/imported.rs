//! What a copy shows of the import that created it (the result `z3vhzo` in Import resources `Dzzks`): the strip
//! over a copied pipeline that is not published yet, and the inspector's Imported, Imported with it and Missing
//! sections. For display only: the copy does not follow its source.

use orchester_core::{
  id::ResourceId,
  import::{Catalog, CatalogResource, Content, Entity, ImportRecord},
  resource::VersionNumber,
  store::ImportStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

/// How a resource came along with the copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Came {
  /// The import created it too; `renamed`: under a name other than its source's.
  Created { renamed: bool },
  /// This project's own, used in place of the source's.
  Reused,
}

/// A resource of this project the import created or used along with the copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithIt {
  pub id: ResourceId,
  pub name: String,
  /// What it holds now: its draft, else its latest published version.
  pub content: Content,
  /// Its latest published version; `None` while it is only a draft.
  pub published: Option<VersionNumber>,
  pub came: Came,
}

/// A connection, MCP server or secret of the source the import left unresolved: the copies keep naming it, and
/// this project does not have it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
  pub source: Entity,
  pub name: String,
}

/// The import that created a copy, as the copy shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Imported {
  pub at: Timestamp,
  /// The project imported from, by the name it had then.
  pub project: String,
  /// The copy's source resource and the version copied; `None` for a draft.
  pub source: String,
  pub version: Option<VersionNumber>,
  /// Everything else the import created or used, that this project still has: what it created first.
  pub with_it: Vec<WithIt>,
  pub missing: Vec<Missing>,
  /// How many drafts the import created, resources, connections and secrets of this project it used, and
  /// items it left unresolved.
  pub created: usize,
  pub reused: usize,
  pub unresolved: usize,
}

impl Imported {
  /// Whether the copy shows the import's result over it: until it is published, nothing of it runs.
  pub const fn shows_result(&self, published: bool) -> bool {
    !published
  }

  /// Whether something was left unresolved, which the project's tools are where to resolve.
  pub const fn needs_setup(&self) -> bool {
    self.unresolved > 0
  }
}

/// What `resource` shows of the import that created it; `None` when no import did.
pub fn imported(db: &ProjectDb, resource: ResourceId) -> Result<Option<Imported>, StoreError> {
  let Some(record) = db.import_record(resource)? else {
    return Ok(None);
  };
  let catalog = Catalog::read(db.project().clone(), db)?;
  Ok(view(&record, &catalog, resource))
}

/// `record` as `resource` shows it, with what this project (`catalog`) holds now.
pub fn view(record: &ImportRecord, catalog: &Catalog, resource: ResourceId) -> Option<Imported> {
  let provenance = record.provenance(resource)?;
  let created = record
    .copies
    .iter()
    .filter(|(copy, _)| *copy != resource)
    .filter_map(|(copy, provenance)| {
      let entry = catalog.resource(*copy)?;
      let renamed = entry.resource().name() != &provenance.resource_name;
      with_it(entry, Came::Created { renamed })
    });
  let reused = record.resolved.iter().filter_map(|resolved| match resolved.target {
    Some(Entity::Resource(id)) => catalog.resource(id).and_then(|entry| with_it(entry, Came::Reused)),
    _ => None,
  });
  let missing = record
    .resolved
    .iter()
    .filter(|resolved| resolved.target.is_none())
    .map(|resolved| Missing {
      source: resolved.source,
      name: resolved.source_name.clone(),
    })
    .collect::<Vec<_>>();
  Some(Imported {
    at: record.imported_at,
    project: provenance.project_name.as_str().to_owned(),
    source: provenance.resource_name.as_str().to_owned(),
    version: provenance.version,
    with_it: created.chain(reused).collect(),
    created: record.copies.len(),
    reused: record.resolved.len() - missing.len(),
    unresolved: missing.len(),
    missing,
  })
}

fn with_it(entry: &CatalogResource, came: Came) -> Option<WithIt> {
  let resource = entry.resource();
  Some(WithIt {
    id: resource.id(),
    name: resource.name().as_str().to_owned(),
    content: entry.content(entry.default_pick())?.clone(),
    published: entry.published(),
    came,
  })
}
