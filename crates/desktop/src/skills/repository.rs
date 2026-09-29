//! Import from repository (ORC-15 `wulRC`): reading what the repository gives its agents —
//! `AGENTS.md` and `CLAUDE.md` at its root, each folder of `.claude/skills` and `.agents/skills` —
//! and turning the chosen items into drafts. The repository is only read. Every function blocks on
//! the disk or SQLite; the UI calls them off the UI thread.

use std::{
  collections::HashSet,
  fs, io,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  agent_files::import::{
    Candidate, Found, FoundFile, INSTRUCTION_FILES, ImportItem, ItemKind, RepositoryDraftError, SKILL_FOLDERS,
    candidates,
  },
  id::IdempotencyKey,
  resource::{Resource, ResourceKind, SKILL_FILE, SkillFile, SkillFilePath, Slug},
  store::{RepositoryImportStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::data::put_file;

/// What the repository gives its agents, as the import lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryScan {
  pub items: Arc<Vec<ImportItem>>,
}

impl lurq::app::component::DevtoolsInspectable for RepositoryScan {}

impl RepositoryScan {
  /// The instruction files found, and how many skill folders.
  pub fn found(&self) -> (Vec<&str>, usize) {
    let files = self
      .items
      .iter()
      .filter(|item| item.kind == ItemKind::Instruction)
      .map(|item| item.source.as_str())
      .collect();
    let skills = self.items.iter().filter(|item| item.kind == ItemKind::Skill).count();
    (files, skills)
  }
}

#[derive(Debug, Error)]
pub enum ScanError {
  #[error("cannot read {path}: {source}")]
  Read { path: PathBuf, source: io::Error },
}

fn read_error(path: &Path) -> impl FnOnce(io::Error) -> ScanError + '_ {
  move |source| ScanError::Read {
    path: path.to_path_buf(),
    source,
  }
}

/// Reads the repository at `repo_root`; nothing found is an empty scan.
pub fn scan(repo_root: &Path) -> Result<RepositoryScan, ScanError> {
  let mut found = Vec::new();
  for file in INSTRUCTION_FILES {
    let path = repo_root.join(file);
    if let Some(content) = read_if_file(&path)? {
      found.push(Found::Instructions {
        path: file.to_owned(),
        content,
      });
    }
  }
  for folder in SKILL_FOLDERS {
    let root = folder
      .split('/')
      .fold(repo_root.to_path_buf(), |path, name| path.join(name));
    for (name, path) in subfolders(&root)? {
      found.push(skill_folder(&format!("{folder}/{name}"), &path)?);
    }
  }
  Ok(RepositoryScan {
    items: Arc::new(candidates(found)),
  })
}

/// The content of the file at `path`; `None` when there is none.
fn read_if_file(path: &Path) -> Result<Option<Vec<u8>>, ScanError> {
  match fs::metadata(path) {
    Ok(metadata) if metadata.is_file() => fs::read(path).map(Some).map_err(read_error(path)),
    Ok(_) => Ok(None),
    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
    Err(error) => Err(read_error(path)(error)),
  }
}

/// The folders in `root`, by name, following links; none when it does not exist.
fn subfolders(root: &Path) -> Result<Vec<(String, PathBuf)>, ScanError> {
  let entries = match fs::read_dir(root) {
    Ok(entries) => entries,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(error) => return Err(read_error(root)(error)),
  };
  let mut folders = Vec::new();
  for entry in entries {
    let path = entry.map_err(read_error(root))?.path();
    // A file, or a link to nothing, is no skill folder.
    if fs::metadata(&path).is_ok_and(|metadata| metadata.is_dir()) {
      folders.push((
        path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        path,
      ));
    }
  }
  folders.sort();
  Ok(folders)
}

/// A skill folder: its `SKILL.md` and every other file under it.
fn skill_folder(source: &str, folder: &Path) -> Result<Found, ScanError> {
  let markdown = read_if_file(&folder.join(SKILL_FILE))?;
  let mut files = Vec::new();
  let mut unsupported = Vec::new();
  let mut found = Collected {
    files: &mut files,
    unsupported: &mut unsupported,
    visited: HashSet::new(),
  };
  found.collect(folder, "")?;
  Ok(Found::Skill {
    folder: source.to_owned(),
    markdown,
    files,
    unsupported,
  })
}

/// The files of a skill folder as they are collected, and the folders read so far.
struct Collected<'a> {
  files: &'a mut Vec<FoundFile>,
  unsupported: &'a mut Vec<String>,
  /// A folder reached again through a link is read once.
  visited: HashSet<PathBuf>,
}

impl Collected<'_> {
  /// The files under `folder` besides the skill's own `SKILL.md`, each with its path from the
  /// skill folder (`prefix` for this folder).
  fn collect(&mut self, folder: &Path, prefix: &str) -> Result<(), ScanError> {
    if !self
      .visited
      .insert(fs::canonicalize(folder).map_err(read_error(folder))?)
    {
      return Ok(());
    }
    let mut entries = fs::read_dir(folder)
      .map_err(read_error(folder))?
      .map(|entry| entry.map(|entry| entry.path()))
      .collect::<Result<Vec<_>, _>>()
      .map_err(read_error(folder))?;
    entries.sort();
    for path in entries {
      let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
      let relative = format!("{prefix}{name}");
      let metadata = fs::metadata(&path).map_err(read_error(&path))?;
      if metadata.is_dir() {
        self.collect(&path, &format!("{relative}/"))?;
      } else if relative != SKILL_FILE {
        match SkillFilePath::new(&relative) {
          Ok(file_path) => self.files.push(FoundFile {
            path: file_path,
            content: Arc::from(fs::read(&path).map_err(read_error(&path))?),
          }),
          Err(_) => self.unsupported.push(relative),
        }
      }
    }
    Ok(())
  }
}

#[derive(Debug, Error)]
pub enum ImportFailure {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Draft(#[from] RepositoryDraftError),
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
