//! Read stamps (`docs/architecture/native-agent.md` §4.1, §4.4): what a file was when the agent last read
//! or wrote it. An edit compares the stamp with the file as it is now, because commands the agent ran
//! may have changed it since.

use std::{
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
  time::SystemTime,
};

use parking_lot::Mutex;
use sha2::{Digest, Sha256};

/// A file's length, modified time and content hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ReadStamp {
  len: u64,
  modified: Option<SystemTime>,
  hash: [u8; 32],
}

impl ReadStamp {
  /// The stamp of `bytes`, just read from or written to a file with `metadata`.
  pub(super) fn of(bytes: &[u8], metadata: &fs::Metadata) -> Self {
    Self {
      len: metadata.len(),
      // A file system without modified times still has the length and the hash to compare.
      modified: metadata.modified().ok(),
      hash: Sha256::digest(bytes).into(),
    }
  }
}

/// The stamps of the files the agent read or wrote, by their resolved path.
#[derive(Debug, Default)]
pub(super) struct Stamps(Mutex<HashMap<PathBuf, ReadStamp>>);

impl Stamps {
  pub(super) fn record(&self, path: &Path, stamp: ReadStamp) {
    self.0.lock().insert(path.to_path_buf(), stamp);
  }

  pub(super) fn get(&self, path: &Path) -> Option<ReadStamp> {
    self.0.lock().get(path).copied()
  }

  pub(super) fn clear(&self) {
    self.0.lock().clear();
  }
}
