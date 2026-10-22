//! Content-addressed blobs (`docs/architecture/storage.md`, "Blobs"): zstd-compressed files at
//! `<repo>/.orchester/blobs/<aa>/<sha256>` with a `blobs` row each. A file is written under a
//! temporary name, read back and checked against its hash, then renamed into place; the row is
//! inserted after the file exists. Equal content is stored once, and blobs are never deleted.

use std::{
  fmt::Write as _,
  fs,
  io::{self, ErrorKind},
  path::{Path, PathBuf},
  process,
  sync::atomic::{AtomicU64, Ordering},
};

use orchester_core::{blob::ContentHash, store::BlobStore, time::Timestamp};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

const DIRECTORY: &str = "blobs";
/// Numbers this process's temporary blob files: two threads writing the same blob at once each write their own.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
/// zstd's default level: fast, and text compresses several times over.
const LEVEL: i32 = 3;

/// The hex SHA-256 of a blob's uncompressed content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BlobHash(String);

impl BlobHash {
  pub(super) fn of(content: &[u8]) -> Self {
    let digest = Sha256::digest(content);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest.iter() {
      write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Self(hex)
  }

  /// A stored hash, checked to be 64 lowercase hex digits so it can name a file.
  pub(super) fn stored(value: String) -> Result<Self, StoreError> {
    let valid = value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if valid {
      Ok(Self(value))
    } else {
      Err(corrupt("blob hash", value))
    }
  }

  pub(super) fn as_str(&self) -> &str {
    &self.0
  }

  fn path(&self, orchester: &Path) -> PathBuf {
    orchester.join(DIRECTORY).join(&self.0[..2]).join(&self.0)
  }
}

/// A blob's file, written; its row is not recorded yet.
pub(super) struct WrittenBlob {
  pub(super) hash: BlobHash,
  size: i64,
  stored_size: i64,
}

/// Writes the file of `content` under `orchester` (`<repo>/.orchester`) unless a blob with the same
/// content exists: what a transaction that refers to the blob records ([`WrittenBlob::record`]).
pub(super) fn write(orchester: &Path, content: &[u8]) -> Result<WrittenBlob, StoreError> {
  let hash = BlobHash::of(content);
  let path = hash.path(orchester);
  if !path.is_file() {
    write_file(&path, &hash, content)?;
  }
  let stored_size = fs::metadata(&path).map_err(StoreError::io(&path))?.len();
  let size = |value: u64| i64::try_from(value).map_err(|_| StoreError::Unrepresentable("blob size"));
  Ok(WrittenBlob {
    size: size(content.len() as u64)?,
    stored_size: size(stored_size)?,
    hash,
  })
}

impl WrittenBlob {
  /// Records the blob's row, as of `media_type`, unless it is recorded already.
  pub(super) fn record(&self, connection: &Connection, media_type: &str, now: Timestamp) -> Result<(), StoreError> {
    connection.execute(
      "INSERT OR IGNORE INTO blobs (sha256, size, stored_size, media_type, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        self.hash.as_str(),
        self.size,
        self.stored_size,
        media_type,
        now.unix_millis()
      ],
    )?;
    Ok(())
  }
}

/// The content of the blob `hash`, checked against its hash.
pub(super) fn get(connection: &Connection, orchester: &Path, hash: &BlobHash) -> Result<Vec<u8>, StoreError> {
  let known: Option<i64> = connection
    .query_row("SELECT size FROM blobs WHERE sha256 = ?1", [hash.as_str()], |row| {
      row.get(0)
    })
    .optional()?;
  if known.is_none() {
    return Err(corrupt("blob", format!("{} has no row", hash.as_str())));
  }
  let path = hash.path(orchester);
  let compressed = fs::read(&path).map_err(StoreError::io(&path))?;
  let content = zstd::decode_all(compressed.as_slice()).map_err(|error| corrupt("blob", error))?;
  if BlobHash::of(&content) != *hash {
    return Err(corrupt("blob", format!("{} does not match its content", hash.as_str())));
  }
  Ok(content)
}

/// Writes the compressed `content` to a temporary file beside `path`, checks it decompresses to
/// `hash`, then renames it into place. Another thread or process may have stored the same blob meanwhile;
/// its file has the same content.
fn write_file(path: &Path, hash: &BlobHash, content: &[u8]) -> Result<(), StoreError> {
  let directory = path.parent().expect("a blob path has its two-letter directory");
  fs::create_dir_all(directory).map_err(StoreError::io(directory))?;
  let compressed = zstd::encode_all(content, LEVEL).map_err(StoreError::io(path))?;
  let number = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
  let temporary = directory.join(format!("{}.{}.{number}.tmp", hash.as_str(), process::id()));
  fs::write(&temporary, &compressed).map_err(StoreError::io(&temporary))?;
  let written = fs::read(&temporary).map_err(StoreError::io(&temporary))?;
  let verified = zstd::decode_all(written.as_slice()).is_ok_and(|read| BlobHash::of(&read) == *hash);
  if !verified {
    remove_temporary(&temporary);
    return Err(StoreError::Io {
      path: temporary,
      source: io::Error::new(ErrorKind::InvalidData, "the written blob does not match its hash"),
    });
  }
  fs::rename(&temporary, path).map_err(|source| {
    remove_temporary(&temporary);
    StoreError::Io {
      path: path.to_path_buf(),
      source,
    }
  })
}

/// Removes a temporary file after a failed write.
fn remove_temporary(temporary: &Path) {
  // A leftover temporary file is never read, only wastes space; the write's own error is the one
  // to report, so a failed removal is dropped.
  let _ = fs::remove_file(temporary);
}

impl BlobStore for ProjectDb {
  type Error = StoreError;

  fn put_blob(&mut self, media_type: &str, content: &[u8]) -> Result<ContentHash, StoreError> {
    let blob = write(&self.directory, content)?;
    blob.record(&self.connection, media_type, Timestamp::now())?;
    ContentHash::new(blob.hash.as_str()).map_err(|error| corrupt("blob hash", error))
  }

  fn blob(&self, hash: &ContentHash) -> Result<Vec<u8>, StoreError> {
    get(
      &self.connection,
      &self.directory,
      &BlobHash::stored(hash.as_str().to_owned())?,
    )
  }
}
