//! Typed local Git execution. Raw bytes stay inside this boundary; errors never include Git or hook output.

mod ancestry;
mod capture;
mod commit;
mod commit_copy;
mod execute;
pub mod family;
mod files;
mod lease;
mod merge_receipt;
pub mod owned;
mod process;
mod resolve;

use std::path::{Path, PathBuf};

pub use files::{
  copy_untracked_file, executable, file_hash, owned_directory, owned_store, prepare_scratch, read_regular,
  set_executable, write_regular,
};
pub use lease::IntegrationLease;
use orchester_core::{
  blob::ContentHash,
  integration::local::{Checkout, LocalError, LocalPath},
};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LocalGitError {
  #[error(transparent)]
  State(#[from] LocalError),
  #[error("cannot execute the local Git operation")]
  Process,
  #[error("Git or a repository hook refused the operation")]
  Refused,
  #[error("cannot access local checkout content")]
  Io,
  #[error("Git returned unsupported repository data")]
  Data,
}

/// A canonical checkout and its own scratch folder, never an arbitrary worktree chosen by Git environment variables.
#[derive(Clone)]
pub struct LocalGit {
  root: PathBuf,
  scratch: PathBuf,
  ref_action: Option<String>,
  _root_owner: std::sync::Arc<owned::Directory>,
  _scratch_owner: std::sync::Arc<owned::Directory>,
  journal: Option<std::sync::Arc<dyn orchester_core::integration::local::ProcessJournal>>,
}

impl LocalGit {
  pub fn new(root: &Path, scratch: &Path) -> Result<Self, LocalGitError> {
    let root_owner = owned::Directory::open(root)?;
    let scratch_owner = owned::Directory::open(scratch)?;
    let root = root.canonicalize().map_err(|_| LocalGitError::Io)?;
    let scratch = scratch.canonicalize().map_err(|_| LocalGitError::Io)?;
    if !scratch.starts_with(&root) {
      return Err(LocalError::Unsupported.into());
    }
    let git = Self {
      root,
      scratch,
      ref_action: None,
      journal: None,
      _root_owner: root_owner,
      _scratch_owner: scratch_owner,
    };
    let top = git.text(&["rev-parse", "--show-toplevel"])?;
    if Path::new(&top).canonicalize().map_err(|_| LocalGitError::Io)? != git.root {
      return Err(LocalError::Unsupported.into());
    }
    Ok(git)
  }

  pub fn bind_journal(&mut self, journal: std::sync::Arc<dyn orchester_core::integration::local::ProcessJournal>) {
    self.journal = Some(journal);
  }

  pub fn root(&self) -> &Path {
    &self.root
  }

  pub fn snapshot(&self) -> Result<Checkout, LocalGitError> {
    self.capture()
  }

  pub fn text(&self, args: &[&str]) -> Result<String, LocalGitError> {
    let output = self.raw(args, None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    String::from_utf8(output.bytes)
      .map(|text| text.trim().to_owned())
      .map_err(|_| LocalGitError::Data)
  }

  pub fn blob(&self, object: &str) -> Result<Vec<u8>, LocalGitError> {
    validate_object(object)?;
    let output = self.raw(&["cat-file", "blob", object], None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    Ok(output.bytes)
  }

  pub fn checkout_blob(&self, tree: &str, path: &LocalPath) -> Result<Vec<u8>, LocalGitError> {
    validate_object(tree)?;
    let object = format!("{tree}:{}", path.as_str());
    let output = self.raw(&["cat-file", "--filters", &object], None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    Ok(output.bytes)
  }

  pub fn put_path_blob(&self, path: &LocalPath, content: &[u8]) -> Result<String, LocalGitError> {
    let filter_path = format!("--path={path}");
    let output = self.raw(&["hash-object", "-w", "--stdin", &filter_path], Some(content), None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let object = String::from_utf8(output.bytes).map_err(|_| LocalGitError::Data)?;
    let object = object.trim();
    validate_object(object)?;
    Ok(object.to_owned())
  }

  pub fn put_blob(&self, content: &[u8]) -> Result<String, LocalGitError> {
    let output = self.raw(&["hash-object", "-w", "--stdin", "--no-filters"], Some(content), None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let object = String::from_utf8(output.bytes).map_err(|_| LocalGitError::Data)?;
    let object = object.trim();
    validate_object(object)?;
    Ok(object.to_owned())
  }

  pub fn diff(&self, staged: bool, paths: &[LocalPath]) -> Result<String, LocalGitError> {
    let mut args = vec!["diff", "--no-color", "--no-ext-diff", "--no-textconv"];
    if staged {
      args.push("--cached");
    }
    args.push("--");
    args.extend(paths.iter().map(LocalPath::as_str));
    self.text(&args)
  }

  pub fn file(&self, path: &LocalPath) -> Result<Option<Vec<u8>>, LocalGitError> {
    read_regular(&self.root, path)
  }

  pub fn ensure(&self, reviewed: &Checkout) -> Result<(), LocalGitError> {
    reviewed.ensure_current(&self.snapshot()?)?;
    Ok(())
  }
}

pub fn digest(bytes: &[u8]) -> ContentHash {
  use std::fmt::Write as _;
  let mut hex = String::with_capacity(64);
  for byte in Sha256::digest(bytes).iter() {
    write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
  }
  ContentHash::new(&hex).expect("SHA-256 produces 64 lowercase hexadecimal digits")
}

fn validate_object(object: &str) -> Result<(), LocalGitError> {
  if !matches!(object.len(), 40 | 64) || !object.bytes().all(|b| b.is_ascii_hexdigit()) {
    return Err(LocalGitError::Data);
  }
  Ok(())
}
