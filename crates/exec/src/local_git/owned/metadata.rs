//! Git uses pathnames internally. While an operation changes the repository, hold its existing metadata directory
//! chains, every existing loose-object bucket among them, against rename and deletion before Git uses them.
//! Nothing is created in `.git`: missing folders are Git's to make. Folders outside `objects` are kept nonempty by
//! an anchor so they cannot be turned into junctions; the anchors are removed when the ownership ends. `objects` is
//! Git's alone and gets no anchor: an empty loose-object bucket is held against rename only.
use std::{
  ffi::OsStr,
  fs::File,
  io::Read,
  path::{Path, PathBuf},
  sync::Arc,
};

use super::{
  Directory, LocalError, LocalGitError,
  windows::{self, Pin},
};

/// The anchor's name, the same in every folder it keeps nonempty.
const ANCHOR: &str = ".orchester-local-owner";

pub struct MetadataOwnership {
  directories: Vec<Arc<Directory>>,
  files: Vec<File>,
  /// The anchors this ownership placed, removed when it ends.
  anchors: Vec<PathBuf>,
  /// The `objects` folder, whose subfolders get no anchor.
  objects: Option<PathBuf>,
}
impl MetadataOwnership {
  pub fn acquire(root: &Arc<Directory>) -> Result<Self, LocalGitError> {
    let mut held = Self {
      directories: Vec::new(),
      files: Vec::new(),
      anchors: Vec::new(),
      objects: None,
    };
    let git = match root.child(OsStr::new(".git"), false) {
      Ok(Some(git)) => git,
      Err(LocalGitError::State(LocalError::Unsupported)) => held.linked(root)?,
      Ok(None) => return Err(LocalError::Unsupported.into()),
      Err(error) => return Err(error),
    };
    held.pin(&git)?;
    let common = match git.open_regular(OsStr::new("commondir"), false, false)? {
      Some(file) => {
        let path = held.read_pointer(file)?;
        if path.trim() != "../.." || git.path().parent().and_then(Path::file_name) != Some(OsStr::new("worktrees")) {
          return Err(LocalError::Unsupported.into());
        }
        Directory::open(&git.path().join(path.trim()))?
      }
      None => git.clone(),
    };
    // Only a standard project-owned common directory and its linked worktree metadata are supported.
    let project = common.path().parent().ok_or(LocalError::Unsupported)?;
    if common.path().file_name() != Some(OsStr::new(".git"))
      || !within(root.path(), project)?
      || !within(git.path(), common.path())?
    {
      return Err(LocalError::Unsupported.into());
    }
    held.pin(&common)?;
    held.objects(&common)?;
    held.refs(&common)?;
    held.ensure_current()?;
    Ok(held)
  }

  fn objects(&mut self, common: &Arc<Directory>) -> Result<(), LocalGitError> {
    let objects = common.child(OsStr::new("objects"), false)?.ok_or(LocalGitError::Io)?;
    self.objects = Some(objects.path().to_owned());
    // Every existing bucket, pack and info folder, held as Git left them.
    self.tree(&objects)?;
    // An alternates file would make objects outside this repository readable: refuse one with content.
    if let Some(info) = objects.child(OsStr::new("info"), false)?
      && let Some(alternates) = info.open_regular(OsStr::new("alternates"), false, false)?
    {
      if alternates.metadata().map_err(|_| LocalGitError::Io)?.len() != 0 {
        return Err(LocalError::Unsupported.into());
      }
      self.files.push(alternates);
    }
    Ok(())
  }
  fn refs(&mut self, common: &Arc<Directory>) -> Result<(), LocalGitError> {
    for name in ["refs", "logs"] {
      if let Some(directory) = common.child(OsStr::new(name), false)? {
        self.tree(&directory)?;
      }
    }
    Ok(())
  }

  fn linked(&mut self, root: &Arc<Directory>) -> Result<Arc<Directory>, LocalGitError> {
    let worktrees = root.path().parent().ok_or(LocalError::Unsupported)?;
    let local = worktrees.parent().ok_or(LocalError::Unsupported)?;
    let project = local.parent().ok_or(LocalError::Unsupported)?;
    if worktrees.file_name() != Some(OsStr::new("worktrees")) || local.file_name() != Some(OsStr::new(".orchester")) {
      return Err(LocalError::Unsupported.into());
    }
    let pointer = self.pointer(root, ".git")?;
    let target = Path::new(pointer.strip_prefix("gitdir: ").ok_or(LocalError::Unsupported)?.trim());
    if !target.is_absolute()
      || target
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
      || !within(target, &project.join(".git/worktrees"))?
    {
      return Err(LocalError::Unsupported.into());
    }
    Directory::open(target)
  }

  fn pointer(&mut self, directory: &Arc<Directory>, name: &str) -> Result<String, LocalGitError> {
    self.read_pointer(
      directory
        .open_regular(OsStr::new(name), false, false)?
        .ok_or(LocalGitError::Io)?,
    )
  }
  fn read_pointer(&mut self, mut file: File) -> Result<String, LocalGitError> {
    windows::validate_regular(&file)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
      .take(4097)
      .read_to_end(&mut bytes)
      .map_err(|_| LocalGitError::Io)?;
    if bytes.len() > 4096 {
      return Err(LocalError::Unsupported.into());
    }
    let text = String::from_utf8(bytes).map_err(|_| LocalError::Unsupported)?;
    self.files.push(file);
    Ok(text)
  }
  fn pin(&mut self, directory: &Arc<Directory>) -> Result<(), LocalGitError> {
    if self.directories.iter().any(|held| held.path() == directory.path()) {
      return Ok(());
    }
    let in_objects = self
      .objects
      .as_ref()
      .is_some_and(|objects| directory.path().starts_with(objects));
    if !in_objects {
      self
        .files
        .push(windows::pin_store_entry(directory, OsStr::new(ANCHOR), Pin::Anchor)?);
      self.anchors.push(directory.path().join(ANCHOR));
    }
    self.directories.push(directory.clone());
    Ok(())
  }
  fn tree(&mut self, directory: &Arc<Directory>) -> Result<(), LocalGitError> {
    self.pin(directory)?;
    for entry in std::fs::read_dir(directory.path()).map_err(|_| LocalGitError::Io)? {
      let entry = entry.map_err(|_| LocalGitError::Io)?;
      let metadata = entry.file_type().map_err(|_| LocalGitError::Io)?;
      if metadata.is_symlink() {
        return Err(LocalError::Unsupported.into());
      }
      if metadata.is_dir() {
        self.tree(&directory.child(&entry.file_name(), false)?.ok_or(LocalError::Stale)?)?;
      } else if entry.file_name() == ANCHOR {
        // Left by an ownership that could not remove it; this one removes it when it ends.
        self.anchors.push(entry.path());
      }
    }
    Ok(())
  }
  pub fn ensure_current(&self) -> Result<(), LocalGitError> {
    for directory in &self.directories {
      directory.ensure_pinned()?;
    }
    for file in &self.files {
      windows::validate_regular(file)?;
    }
    Ok(())
  }
}

impl Drop for MetadataOwnership {
  fn drop(&mut self) {
    // The anchors' handles deny deletion: close them before removing the files.
    self.files.clear();
    for anchor in &self.anchors {
      match std::fs::remove_file(anchor) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        // A drop cannot return the failure; the next ownership of this repository finds and removes the anchor.
        Err(error) => tracing::warn!(%error, "cannot remove a local integration anchor"),
      }
    }
  }
}
fn within(path: &Path, parent: &Path) -> Result<bool, LocalGitError> {
  // Git writes normal DOS paths while canonical Windows paths may have a verbatim prefix.
  // This compares names only; opened no-reparse chains supply the ownership proof.
  let path = super::super::commit_copy::git_name(path)?.replace('\\', "/");
  let parent = super::super::commit_copy::git_name(parent)?.replace('\\', "/");
  let mut path = path;
  let mut parent = parent;
  for name in [&mut path, &mut parent] {
    if name.as_bytes().get(1) == Some(&b':') && name.as_bytes()[0].is_ascii_alphabetic() {
      name.replace_range(..1, &name[..1].to_ascii_uppercase());
    }
  }
  let parent = parent.trim_end_matches('/');
  Ok(path == parent || path.starts_with(&format!("{parent}/")))
}
