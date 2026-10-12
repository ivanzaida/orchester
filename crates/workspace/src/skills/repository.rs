//! Import from repository (ORC-15 `wulRC`): reading what the repository gives its agents —
//! `AGENTS.md` and `CLAUDE.md` at its root, each folder of `.claude/skills` and `.agents/skills` —
//! and turning the chosen items into drafts. The repository is only read. Every function blocks on
//! the disk or SQLite; the UI calls them off the UI thread.

mod scan;
use std::path::Path;

use orchester_core::{
  agent_files::import::{Candidate, ImportItem, RepositoryDraftError},
  id::IdempotencyKey,
  resource::{Resource, ResourceKind, SkillFile, Slug},
  store::{RepositoryImportStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
pub use scan::{RepositoryScan, ScanError, scan};
use thiserror::Error;

use super::data::put_file;

#[derive(Debug, Error)]
pub enum ImportFailure {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Draft(#[from] RepositoryDraftError),
}
impl ImportFailure {
  pub fn message(&self) -> crate::messages::Message {
    match self {
      Self::Store(e) => crate::messages::store_error(e),
      Self::Draft(_) => crate::messages::Message::new("skills.library.loadFailed"),
    }
  }
}

/// What an import made of one chosen item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedDraft {
  pub resource: Resource,
  /// The file or folder it was made from.
  pub source: String,
  /// Reference files of a skill; none for an instruction.
  pub files: usize,
}

/// Creates a draft for each of `chosen`, each reference file stored among the project's blobs
/// first, all under `key`: repeating it creates nothing new.
pub fn create_drafts_in(
  repo_root: &Path,
  key: IdempotencyKey,
  chosen: &[ImportItem],
) -> Result<Vec<CreatedDraft>, ImportFailure> {
  let mut db = ProjectDb::open(repo_root)?;
  let slugs = |db: &ProjectDb, kind| -> Result<Vec<Slug>, StoreError> {
    Ok(
      db.resources(kind)?
        .iter()
        .map(|resource| resource.slug().clone())
        .collect(),
    )
  };
  let mut instructions = slugs(&db, ResourceKind::Instruction)?;
  let mut skills = slugs(&db, ResourceKind::Skill)?;
  let mut drafts = Vec::new();
  let mut files = Vec::new();
  for item in chosen {
    let Ok(candidate) = &item.outcome else {
      continue;
    };
    let draft = match candidate {
      Candidate::Instruction { .. } => candidate.draft(&item.source, &mut instructions, Vec::new())?,
      Candidate::Skill { files: found, .. } => {
        let stored = found
          .iter()
          .map(|file| {
            Ok(SkillFile {
              path: file.path.clone(),
              content: put_file(&mut db, &file.content)?,
              size: file.content.len() as u64,
            })
          })
          .collect::<Result<Vec<_>, StoreError>>()?;
        candidate.draft(&item.source, &mut skills, stored)?
      }
    };
    files.push((
      item.source.clone(),
      match candidate {
        Candidate::Skill { files, .. } => files.len(),
        Candidate::Instruction { .. } => 0,
      },
    ));
    drafts.push(draft);
  }
  let created = db.import_repository_drafts(key, &drafts, Timestamp::now())?;
  Ok(
    created
      .into_iter()
      .zip(files)
      .map(|(resource, (source, files))| CreatedDraft {
        resource,
        source,
        files,
      })
      .collect(),
  )
}
