//! Read-only repository discovery. Resolve ownership before traversing or reading any entry.
use std::{
  collections::HashSet,
  fs::{self, File},
  io::{self, Read},
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  agent_files::import::{Found, FoundFile, INSTRUCTION_FILES, ImportItem, ItemKind, SKILL_FOLDERS, candidates},
  native::scope::{FileScope, ScopeError, ScopeRoots},
  permission::RepositoryAccess,
  resource::{SKILL_FILE, SkillFilePath},
};
use thiserror::Error;

const MAX_DEPTH: usize = 8;
const MAX_ENTRIES: usize = 4096;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_SCAN_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryScan {
  pub items: Arc<Vec<ImportItem>>,
}
impl RepositoryScan {
  pub fn found(&self) -> (Vec<&str>, usize) {
    let files = self
      .items
      .iter()
      .filter(|i| i.kind == ItemKind::Instruction)
      .map(|i| i.source.as_str())
      .collect();
    let skills = self.items.iter().filter(|i| i.kind == ItemKind::Skill).count();
    (files, skills)
  }
}

#[derive(Debug, Error)]
pub enum ScanError {
  #[error("cannot read {path}: {source}")]
  Read { path: PathBuf, source: io::Error },
  #[error("{0} leads outside its repository or skill folder")]
  Outside(PathBuf),
  #[error("{0} leads into protected repository state")]
  Protected(PathBuf),
  #[error("{0} is not a regular file or directory")]
  NotRegular(PathBuf),
  #[error("repository discovery exceeds its size or traversal bounds at {0}")]
  Bounds(PathBuf),
}
fn read_error(path: &Path) -> impl FnOnce(io::Error) -> ScanError + '_ {
  move |source| ScanError::Read {
    path: path.to_path_buf(),
    source,
  }
}
fn scope_error(path: &Path, error: &ScopeError) -> ScanError {
  match error {
    ScopeError::Protected(_) => ScanError::Protected(path.to_path_buf()),
    ScopeError::Unresolved { kind, .. } => read_error(path)(io::Error::from(*kind)),
    _ => ScanError::Outside(path.to_path_buf()),
  }
}
fn scope(root: &Path) -> Result<FileScope, ScanError> {
  FileScope::new(&ScopeRoots {
    worktree: root,
    access: RepositoryAccess::ReadOnly,
    code_index: None,
    skills: None,
    writable_folders: &[],
  })
  .map_err(|error| scope_error(root, &error))
}

struct Scan {
  project: FileScope,
  entries: usize,
  bytes: u64,
}
struct Collected {
  scope: FileScope,
  base: PathBuf,
  visited: HashSet<PathBuf>,
  files: Vec<FoundFile>,
  unsupported: Vec<String>,
}
impl Scan {
  fn resolve(&self, path: &Path, skill: Option<&FileScope>) -> Result<PathBuf, ScanError> {
    let text = path.to_string_lossy();
    let resolved = self
      .project
      .repository_readable(&text)
      .map_err(|e| scope_error(path, &e))?;
    if let Some(skill) = skill {
      skill.readable(&text).map_err(|e| scope_error(path, &e))?;
    }
    Ok(resolved)
  }
  fn metadata(&self, path: &Path, skill: Option<&FileScope>) -> Result<Option<(PathBuf, fs::Metadata)>, ScanError> {
    let resolved = self.resolve(path, skill)?;
    match fs::metadata(&resolved) {
      Ok(metadata) => Ok(Some((resolved, metadata))),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
      Err(error) => Err(read_error(path)(error)),
    }
  }
  fn read(&mut self, path: &Path, skill: Option<&FileScope>) -> Result<Option<Vec<u8>>, ScanError> {
    let Some((resolved, metadata)) = self.metadata(path, skill)? else {
      return Ok(None);
    };
    if !metadata.is_file() {
      return Err(ScanError::NotRegular(path.to_path_buf()));
    }
    let limit = MAX_FILE_BYTES.min(MAX_SCAN_BYTES - self.bytes);
    if metadata.len() > limit {
      return Err(ScanError::Bounds(path.to_path_buf()));
    }
    let mut content = Vec::new();
    File::open(&resolved)
      .and_then(|file| file.take(limit + 1).read_to_end(&mut content))
      .map_err(read_error(path))?;
    if content.len() as u64 > limit {
      return Err(ScanError::Bounds(path.to_path_buf()));
    }
    self.bytes += content.len() as u64;
    Ok(Some(content))
  }
  fn children(&mut self, path: &Path, skill: Option<&FileScope>, depth: usize) -> Result<Vec<PathBuf>, ScanError> {
    if depth > MAX_DEPTH {
      return Err(ScanError::Bounds(path.to_path_buf()));
    }
    let Some((resolved, metadata)) = self.metadata(path, skill)? else {
      return Ok(Vec::new());
    };
    if !metadata.is_dir() {
      return Err(ScanError::NotRegular(path.to_path_buf()));
    }
    let mut children = Vec::new();
    for entry in fs::read_dir(&resolved).map_err(read_error(path))? {
      self.entries += 1;
      if self.entries > MAX_ENTRIES {
        return Err(ScanError::Bounds(path.to_path_buf()));
      }
      let name = entry.map_err(read_error(path))?.file_name();
      children.push(path.join(name));
    }
    children.sort();
    Ok(children)
  }
  fn skill(&mut self, source: String, folder: &Path) -> Result<Found, ScanError> {
    let root = self.resolve(folder, None)?;
    let skill = scope(&root)?;
    let markdown = self.read(&folder.join(SKILL_FILE), Some(&skill))?;
    let mut collected = Collected {
      scope: skill,
      base: folder.to_path_buf(),
      visited: HashSet::new(),
      files: Vec::new(),
      unsupported: Vec::new(),
    };
    self.collect(folder, 0, &mut collected)?;
    Ok(Found::Skill {
      folder: source,
      markdown,
      files: collected.files,
      unsupported: collected.unsupported,
    })
  }
  fn collect(&mut self, folder: &Path, depth: usize, collected: &mut Collected) -> Result<(), ScanError> {
    if !collected.visited.insert(self.resolve(folder, Some(&collected.scope))?) {
      return Ok(());
    }
    for path in self.children(folder, Some(&collected.scope), depth)? {
      let Some((_, metadata)) = self.metadata(&path, Some(&collected.scope))? else {
        continue;
      };
      if metadata.is_dir() {
        self.collect(&path, depth + 1, collected)?;
        continue;
      }
      let relative = path
        .strip_prefix(&collected.base)
        .expect("children lie under their listed skill path")
        .to_string_lossy()
        .replace('\\', "/");
      if relative == SKILL_FILE {
        continue;
      }
      if !metadata.is_file() {
        return Err(ScanError::NotRegular(path));
      }
      match SkillFilePath::new(&relative) {
        Ok(file_path) => {
          if let Some(content) = self.read(&path, Some(&collected.scope))? {
            collected.files.push(FoundFile {
              path: file_path,
              content: Arc::from(content),
            });
          }
        }
        Err(_) => collected.unsupported.push(relative),
      }
    }
    Ok(())
  }
}

/// Reads only bounded, regular files owned by the canonical repository and each canonical skill.
pub fn scan(repo_root: &Path) -> Result<RepositoryScan, ScanError> {
  let mut scan = Scan {
    project: scope(repo_root)?,
    entries: 0,
    bytes: 0,
  };
  let root = scan.project.worktree().to_path_buf();
  let mut found = Vec::new();
  for file in INSTRUCTION_FILES {
    if let Some(content) = scan.read(&root.join(file), None)? {
      found.push(Found::Instructions {
        path: file.to_owned(),
        content,
      });
    }
  }
  for folder in SKILL_FOLDERS {
    let container = root.join(folder);
    for path in scan.children(&container, None, 0)? {
      let Some((_, metadata)) = scan.metadata(&path, None)? else {
        continue;
      };
      if metadata.is_dir() {
        let name = path.file_name().expect("directory entry has a name").to_string_lossy();
        found.push(scan.skill(format!("{folder}/{name}"), &path)?);
      } else if !metadata.is_file() {
        return Err(ScanError::NotRegular(path));
      }
    }
  }
  Ok(RepositoryScan {
    items: Arc::new(candidates(found)),
  })
}
