//! A discovery root is held open. Canonical links are resolved before entry; subsequent opens never follow links.
//! Unix uses the same openat/no-follow machinery as agent file-entry operations. Windows holds each plain parent
//! without write/delete sharing and opens/enumerates relative to those handles, never a descendant pathname.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
use std::{
  ffi::OsString,
  fs::{File, Metadata},
  io,
  path::{Component, Path, PathBuf},
  sync::Arc,
};

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

pub(super) struct Entry {
  pub file: Arc<File>,
  pub metadata: Metadata,
  path: PathBuf,
  parents: Vec<Arc<File>>,
}
pub(super) struct Directory(Entry);
impl Directory {
  pub fn root(path: &Path) -> io::Result<Self> {
    let mut prefix = PathBuf::new();
    let mut held = Vec::new();
    for part in path.components() {
      prefix.push(part);
      if matches!(part, Component::Normal(_) | Component::RootDir) {
        let file = Arc::new(platform::root_part(&prefix, held.last().map(Arc::as_ref))?);
        platform::directory(&file)?;
        held.push(file);
      }
    }
    let file = held.pop().ok_or_else(changed)?;
    Ok(Self(Entry {
      metadata: file.metadata()?,
      file,
      path: path.to_owned(),
      parents: held,
    }))
  }
  pub fn from_entry(entry: Entry) -> io::Result<Self> {
    platform::directory(&entry.file)?;
    Ok(Self(entry))
  }
  pub fn path(&self) -> &Path {
    &self.0.path
  }
  pub fn open(&self, resolved: &Path) -> io::Result<Entry> {
    let relative = resolved.strip_prefix(self.path()).map_err(|_| changed())?;
    let mut parents = self.0.parents.clone();
    parents.push(self.0.file.clone());
    let mut path = self.path().to_owned();
    let parts = relative.components().collect::<Vec<_>>();
    for (index, part) in parts.iter().enumerate() {
      let Component::Normal(name) = part else {
        return Err(changed());
      };
      path.push(name);
      let file = Arc::new(platform::child(parents.last().expect("root is held"), name, &path)?);
      if index + 1 == parts.len() {
        let metadata = platform::metadata(&file)?;
        return Ok(Entry {
          file,
          metadata,
          path,
          parents,
        });
      }
      platform::directory(&file)?;
      parents.push(file);
    }
    let file = parents.pop().expect("root is held");
    Ok(Entry {
      metadata: file.metadata()?,
      file,
      path,
      parents,
    })
  }
  pub fn names(&self, maximum: usize) -> io::Result<Vec<OsString>> {
    platform::names(&self.0.file, self.path(), maximum)
  }
}
pub(super) fn changed() -> io::Error {
  io::Error::new(io::ErrorKind::InvalidData, "discovery entry or parent changed type")
}
