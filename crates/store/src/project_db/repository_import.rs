//! Import from repository: the drafts made from the repository's own agent files, written in one
//! transaction, and the file or folder each came from (`repository_imports`).

use orchester_core::{
  agent_files::import::{DraftContent, RepositoryDraft, RepositoryOrigin},
  id::{IdempotencyKey, ResourceId},
  resource::{Resource, ResourceKind},
  store::RepositoryImportStore,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{
  ProjectDb,
  resources::{encode, kind_text, load_resources},
};
use crate::StoreError;

impl RepositoryImportStore for ProjectDb {
  type Error = StoreError;

  fn import_repository_drafts(
    &mut self,
    key: IdempotencyKey,
    drafts: &[RepositoryDraft],
    now: Timestamp,
  ) -> Result<Vec<Resource>, StoreError> {
    let bodies = drafts
      .iter()
      .map(|draft| match &draft.content {
        DraftContent::Instruction(body) => encode(body),
        DraftContent::Skill(body) => encode(body),
      })
      .collect::<Result<Vec<_>, _>>()?;
    let transaction = self.connection.transaction()?;
    let earlier = load_resources(
      &transaction,
      "WHERE id IN (SELECT resource_id FROM repository_imports WHERE import_key = ?1) ORDER BY rowid",
      [key.to_string()],
    )?;
    if !earlier.is_empty() {
      return Ok(earlier);
    }
    let mut created = Vec::with_capacity(drafts.len());
    for (draft, body) in drafts.iter().zip(&bodies) {
      created.push(insert(&transaction, key, draft, body, now)?);
    }
    transaction.commit()?;
    Ok(created)
  }

  fn repository_origin(&self, resource: ResourceId) -> Result<Option<RepositoryOrigin>, StoreError> {
    let row: Option<(String, i64)> = self
      .connection
      .query_row(
        "SELECT source, imported_at FROM repository_imports WHERE resource_id = ?1",
        [resource.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    Ok(row.map(|(source, imported_at)| RepositoryOrigin {
      source,
      imported_at: Timestamp::from_unix_millis(imported_at),
    }))
  }
}

/// Stores `draft` as a new resource whose only content is its first draft, with where it came from.
fn insert(
  transaction: &Transaction<'_>,
  key: IdempotencyKey,
  draft: &RepositoryDraft,
  body: &str,
  now: Timestamp,
) -> Result<Resource, StoreError> {
  let kind = match draft.content {
    DraftContent::Instruction(_) => ResourceKind::Instruction,
    DraftContent::Skill(_) => ResourceKind::Skill,
  };
  let taken: bool = transaction.query_row(
    "SELECT EXISTS (SELECT 1 FROM resources WHERE kind = ?1 AND slug = ?2)",
    params![kind_text(kind), draft.slug.as_str()],
    |row| row.get(0),
  )?;
  if taken {
    return Err(StoreError::DuplicateSlug {
      kind,
      slug: draft.slug.as_str().to_owned(),
    });
  }
  let resource = Resource::new(
    ResourceId::generate(),
    kind,
    draft.slug.clone(),
    draft.name.clone(),
    now,
  );
  let id = resource.id().to_string();
  transaction.execute(
    "INSERT INTO resources (id, kind, slug, name, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    params![
      id,
      kind_text(kind),
      draft.slug.as_str(),
      draft.name.as_str(),
      now.unix_millis()
    ],
  )?;
  transaction.execute(
    "INSERT INTO resource_drafts (resource_id, body, based_on, updated_at) VALUES (?1, ?2, NULL, ?3)",
    params![id, body, now.unix_millis()],
  )?;
  transaction.execute(
    "INSERT INTO repository_imports (resource_id, import_key, source, imported_at) VALUES (?1, ?2, ?3, ?4)",
    params![id, key.to_string(), draft.source, now.unix_millis()],
  )?;
  Ok(resource)
}
