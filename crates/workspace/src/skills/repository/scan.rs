//! Bounded discovery through held owned objects; canonical links are resolved before no-follow opens.
use std::{
  collections::HashSet,
  io::{self, Read},
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  agent_files::import::{
    Candidate, Found, FoundFile, INSTRUCTION_FILES, ImportItem, ItemKind, SKILL_FOLDERS, candidates,
  },
  name::Name,
  native::scope::{FileScope, ScopeError, ScopeRoots},
  permission::RepositoryAccess,
  resource::{SKILL_FILE, SkillFilePath},
};
use thiserror::Error;

use super::owned::{Directory, Entry};

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

  /// The instruction files that can be imported, and how many skills folders (`.claude/skills`, `.agents/skills`)
  /// hold the skills found (*AGENTS.md and 2 skills folders*, `HMf2M` in 03).
  pub fn sources(&self) -> (Vec<&str>, usize) {
    let files = self
      .items
      .iter()
      .filter(|item| item.kind == ItemKind::Instruction && item.outcome.is_ok())
      .map(|item| item.source.as_str())
      .collect();
    let folders = SKILL_FOLDERS
      .iter()
      .filter(|folder| {
        self.items.iter().any(|item| {
          item.kind == ItemKind::Skill
            && item
              .source
              .strip_prefix(**folder)
              .is_some_and(|rest| rest.starts_with('/'))
        })
      })
      .count();
    (files, folders)
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
    run_storage: None,
  })
  .map_err(|error| scope_error(root, &error))
}

/// Private-fixture synchronization points before opening, reading or enumerating an owned object.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessPoint {
  Open,
  Read,
  Enumerate,
  ListedEntry,
}
struct Scan<'a> {
  project: FileScope,
  root: Directory,
  observer: AccessObserver<'a>,
  entries: usize,
  bytes: u64,
}
struct Collected {
  root: Directory,
  base: PathBuf,
  visited: HashSet<PathBuf>,
  files: Vec<FoundFile>,
  unsupported: Vec<String>,
}
impl Scan<'_> {
  fn observe(&self, point: AccessPoint, path: &Path) {
    if let Some(observer) = self.observer {
      observer(point, path);
    }
  }
  fn resolve(&self, path: &Path, skill: Option<&Directory>) -> Result<PathBuf, ScanError> {
    let resolved = self
      .project
      .repository_readable(&path.to_string_lossy())
      .map_err(|e| scope_error(path, &e))?;
    if skill.is_some_and(|root| !resolved.starts_with(root.path())) {
      return Err(ScanError::Outside(path.to_owned()));
    }
    Ok(resolved)
  }
  fn entry(&self, path: &Path, skill: Option<&Directory>) -> Result<Option<Entry>, ScanError> {
    let resolved = self.resolve(path, skill)?;
    self.observe(AccessPoint::Open, path);
    match skill.unwrap_or(&self.root).open(&resolved) {
      Ok(entry) => Ok(Some(entry)),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
      Err(error) if error.kind() == io::ErrorKind::InvalidData => Err(ScanError::NotRegular(path.to_owned())),
      Err(error) => Err(read_error(path)(error)),
    }
  }
  fn read(&mut self, path: &Path, skill: Option<&Directory>) -> Result<Option<Vec<u8>>, ScanError> {
    let Some(entry) = self.entry(path, skill)? else {
      return Ok(None);
    };
    if !entry.metadata.is_file() {
      return Err(ScanError::NotRegular(path.to_owned()));
    }
    let limit = MAX_FILE_BYTES.min(MAX_SCAN_BYTES - self.bytes);
    if entry.metadata.len() > limit {
      return Err(ScanError::Bounds(path.to_owned()));
    }
    self.observe(AccessPoint::Read, path);
    let mut content = Vec::new();
    entry
      .file
      .as_ref()
      .take(limit + 1)
      .read_to_end(&mut content)
      .map_err(read_error(path))?;
    if content.len() as u64 > limit {
      return Err(ScanError::Bounds(path.to_owned()));
    }
    self.bytes += content.len() as u64;
    Ok(Some(content))
  }
  fn children(&mut self, path: &Path, skill: Option<&Directory>, depth: usize) -> Result<Vec<PathBuf>, ScanError> {
    if depth > MAX_DEPTH {
      return Err(ScanError::Bounds(path.to_owned()));
    }
    let Some(entry) = self.entry(path, skill)? else {
      return Ok(Vec::new());
    };
    let directory = Directory::from_entry(entry).map_err(|_| ScanError::NotRegular(path.to_owned()))?;
    self.observe(AccessPoint::Enumerate, path);
    let names = directory.names(MAX_ENTRIES - self.entries).map_err(|error| {
      if error.kind() == io::ErrorKind::FileTooLarge {
        ScanError::Bounds(path.to_owned())
      } else {
        read_error(path)(error)
      }
    })?;
    self.entries += names.len();
    for name in &names {
      self.observe(AccessPoint::ListedEntry, &path.join(name));
    }
    let mut children = names.into_iter().map(|name| path.join(name)).collect::<Vec<_>>();
    children.sort();
    Ok(children)
  }
  fn skill(&mut self, source: String, folder: &Path) -> Result<Found, ScanError> {
    let Some(entry) = self.entry(folder, None)? else {
      return Err(read_error(folder)(io::ErrorKind::NotFound.into()));
    };
    let skill = Directory::from_entry(entry).map_err(|_| ScanError::NotRegular(folder.to_owned()))?;
    let markdown = self.read(&folder.join(SKILL_FILE), Some(&skill))?;
    let mut collected = Collected {
      root: skill,
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
    if !collected.visited.insert(self.resolve(folder, Some(&collected.root))?) {
      return Ok(());
    }
    for path in self.children(folder, Some(&collected.root), depth)? {
      let Some(entry) = self.entry(&path, Some(&collected.root))? else {
        continue;
      };
      if entry.metadata.is_dir() {
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
      if !entry.metadata.is_file() {
        return Err(ScanError::NotRegular(path));
      }
      match SkillFilePath::new(&relative) {
        Ok(file_path) => {
          if let Some(content) = self.read(&path, Some(&collected.root))? {
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
  scan_observing(repo_root, None)
}
/// Synchronizes deterministic disposable swaps without replacing filesystem reads or their outcomes.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub fn scan_with_access_observer(
  repo_root: &Path,
  observer: &dyn Fn(AccessPoint, &Path),
) -> Result<RepositoryScan, ScanError> {
  scan_observing(repo_root, Some(observer))
}
type AccessObserver<'a> = Option<&'a dyn Fn(AccessPoint, &Path)>;
fn scan_observing(repo_root: &Path, observer: AccessObserver<'_>) -> Result<RepositoryScan, ScanError> {
  let project = scope(repo_root)?;
  let root = Directory::root(project.worktree()).map_err(read_error(repo_root))?;
  let mut scan = Scan {
    project,
    root,
    observer,
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
      let Some(entry) = scan.entry(&path, None)? else {
        continue;
      };
      if entry.metadata.is_dir() {
        let name = path.file_name().expect("directory entry has a name").to_string_lossy();
        found.push(scan.skill(format!("{folder}/{name}"), &path)?);
      } else if !entry.metadata.is_file() {
        return Err(ScanError::NotRegular(path));
      }
    }
  }
  let mut items = candidates(found);
  items.iter_mut().for_each(name_by_stem);
  Ok(RepositoryScan { items: Arc::new(items) })
}

/// An instruction file's draft is named by its stem, *AGENTS* for `AGENTS.md`, on the Skills and the Instructions
/// side alike (`cC55E`, `P21bwh`); its source keeps the file's path.
fn name_by_stem(item: &mut ImportItem) {
  if let Ok(Candidate::Instruction { name, .. }) = &mut item.outcome
    && let Some(stem) = name.as_str().strip_suffix(".md")
  {
    *name = Name::new(stem).expect("recognized root instruction filenames have a nonempty stem");
  }
}
