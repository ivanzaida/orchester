//! Typed local Git execution. Raw bytes stay inside this boundary; errors never include Git or hook output.

mod abort;
mod ancestry;
mod capture;
mod commit;
mod commit_copy;
mod commit_hooks;
mod discard;
mod execute;
pub mod family;
mod files;
mod ignore_preview;
mod lease;
mod merge_receipt;
pub mod owned;
mod path_collision;
mod process;
mod resolve;
mod sync_preview;

use std::path::{Path, PathBuf};

pub use discard::{DiscardError, Take};
pub use files::{
  copy_untracked_file, executable, file_hash, file_probe, owned_directory, owned_store, prepare_scratch, read_regular,
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
  #[error("a commit hook changed the reviewed commit; nothing was committed")]
  HookChanged(orchester_core::integration::local::HookChange),
  #[error("Git returned unsupported repository data")]
  Data,
}

/// A canonical checkout and its own scratch folder, never an arbitrary worktree chosen by Git environment variables.
#[derive(Clone)]
pub struct LocalGit {
  root: PathBuf,
  scratch: PathBuf,
  git_dir: Option<PathBuf>,
  /// The Git directory as `rev-parse --absolute-git-dir` names it, the form a checkout's identity records.
  git_dir_text: String,
  /// The common Git directory, which a linked worktree shares with its main checkout.
  common_dir_text: String,
  /// The hook directory the commit copy runs hooks from, resolved on first use.
  hooks: std::sync::OnceLock<PathBuf>,
  _git_owner: Option<std::sync::Arc<owned::Directory>>,
  ref_action: Option<String>,
  #[cfg(windows)]
  _execution_owner: std::sync::Arc<owned::ExecutionOwnership>,
  /// The repository's Git metadata, held while an operation changes it ([`Self::own_metadata`]).
  #[cfg(windows)]
  metadata_owner: Option<std::sync::Arc<owned::MetadataOwnership>>,
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
    #[cfg(windows)]
    let execution_owner = owned::ExecutionOwnership::acquire(&root_owner, &scratch_owner)?;
    let mut git = Self {
      root,
      scratch,
      git_dir: None,
      git_dir_text: String::new(),
      common_dir_text: String::new(),
      hooks: std::sync::OnceLock::new(),
      _git_owner: None,
      ref_action: None,
      #[cfg(windows)]
      _execution_owner: execution_owner,
      #[cfg(windows)]
      metadata_owner: None,
      journal: None,
      _root_owner: root_owner,
      _scratch_owner: scratch_owner,
    };
    let located = git.text(&[
      "rev-parse",
      "--show-toplevel",
      "--absolute-git-dir",
      "--path-format=absolute",
      "--git-common-dir",
    ])?;
    let mut lines = located.lines().map(|line| line.trim_end_matches('\r'));
    let (Some(top), Some(named), Some(common), None) = (lines.next(), lines.next(), lines.next(), lines.next()) else {
      return Err(LocalGitError::Data);
    };
    let (named, common) = (named.to_owned(), common.to_owned());
    if Path::new(top).canonicalize().map_err(|_| LocalGitError::Io)? != git.root {
      return Err(LocalError::Unsupported.into());
    }
    let directory = Path::new(&named).canonicalize().map_err(|_| LocalGitError::Io)?;
    git._git_owner = Some(owned::Directory::open(&directory)?);
    git.git_dir = Some(directory);
    git.git_dir_text = named;
    git.common_dir_text = common;
    Ok(git)
  }

  /// Holds the repository's Git metadata against substitution for an operation that changes it, until this
  /// checkout is dropped. Reading needs none of it and writes nothing into `.git`.
  pub fn own_metadata(&mut self) -> Result<(), LocalGitError> {
    #[cfg(windows)]
    if self.metadata_owner.is_none() {
      self.metadata_owner = Some(std::sync::Arc::new(owned::MetadataOwnership::acquire(
        &self._root_owner,
      )?));
    }
    Ok(())
  }

  pub fn bind_journal(&mut self, journal: std::sync::Arc<dyn orchester_core::integration::local::ProcessJournal>) {
    self.journal = Some(journal);
  }

  pub fn root(&self) -> &Path {
    &self.root
  }

  /// The absolute common Git directory, as `rev-parse --git-common-dir` named it when this checkout was opened.
  pub fn common_dir(&self) -> &str {
    &self.common_dir_text
  }

  /// The checkout as its own index sees it: cheap enough to re-read for display.
  pub fn snapshot(&self) -> Result<Checkout, LocalGitError> {
    self.capture(capture::Freshness::Cached)
  }

  /// The checkout with every tracked file hashed: taken right before Git or Orchester changes it.
  pub fn verified_snapshot(&self) -> Result<Checkout, LocalGitError> {
    self.capture(capture::Freshness::Verified)
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

  /// Refreshes the stat data of the checkout's index entries whose content matches, as `git status` would if it
  /// could take the lock: Git's own merge and abort refuse entries written without stat data ("not uptodate").
  /// Changed files stay changed.
  pub fn refresh_index(&self) -> Result<(), LocalGitError> {
    let output = self.quiet_plumbing(&["update-index", "-q", "--refresh"], None, None)?;
    // 1 says some entries differ from their files, which only means they stay changed.
    if !matches!(output.code, Some(0 | 1)) {
      return Err(LocalGitError::Refused);
    }
    Ok(())
  }

  /// Whether the checkout is still exactly `reviewed`, every tracked file hashed.
  pub fn ensure(&self, reviewed: &Checkout) -> Result<(), LocalGitError> {
    reviewed.ensure_current(&self.verified_snapshot()?)?;
    Ok(())
  }
}

/// [`digest`] of everything `read` yields, read in chunks.
pub(crate) fn digest_reader(mut read: impl std::io::Read) -> std::io::Result<ContentHash> {
  let mut hasher = Sha256::new();
  let mut buffer = vec![0; 64 * 1024];
  loop {
    let count = read.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    hasher.update(&buffer[..count]);
  }
  Ok(hex_hash(&hasher.finalize()))
}

pub fn digest(bytes: &[u8]) -> ContentHash {
  hex_hash(&Sha256::digest(bytes))
}

fn hex_hash(bytes: &[u8]) -> ContentHash {
  use std::fmt::Write as _;
  let mut hex = String::with_capacity(64);
  for byte in bytes {
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
