//! Content digests of an index's files, kept in its `manifest.json`. An index is shared by the runs
//! of its revision and its outline files by later indexes, so a file an agent changed would spread:
//! an index is checked against its digests before it is reused, handed to an agent or built on,
//! and one that does not match is built again instead.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  io::{self, Write},
  num::NonZeroUsize,
  path::Path,
  thread,
};

use sha2::{Digest, Sha256};

/// The SHA-256 of each file of an index but its manifest, by its path in the index (`tree.txt`,
/// `outline/src/lib.rs.md`).
pub(crate) type Digests = BTreeMap<String, String>;

const MANIFEST: &str = "manifest.json";

/// The hex SHA-256 of `bytes`.
pub(crate) fn of(bytes: &[u8]) -> String {
  hex(Sha256::digest(bytes).as_slice())
}

fn hex(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A writer that hashes what goes through it.
pub(crate) struct Hashing<W> {
  inner: W,
  hasher: Sha256,
}

impl<W: Write> Hashing<W> {
  pub(crate) fn new(inner: W) -> Self {
    Self {
      inner,
      hasher: Sha256::new(),
    }
  }

  /// The inner writer, flushed, and the digest of everything written.
  pub(crate) fn finish(mut self) -> io::Result<String> {
    self.inner.flush()?;
    Ok(hex(self.hasher.finalize().as_slice()))
  }
}

impl<W: Write> Write for Hashing<W> {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    let written = self.inner.write(bytes)?;
    self.hasher.update(&bytes[..written]);
    Ok(written)
  }

  fn flush(&mut self) -> io::Result<()> {
    self.inner.flush()
  }
}

/// Whether the index in `dir` holds exactly the files of `digests` with that content; why not
/// when it does not. The files are hashed on a thread per core.
pub(crate) fn verify(dir: &Path, digests: &Digests) -> Result<(), String> {
  let mut found = BTreeSet::new();
  list(dir, dir, &mut found).map_err(|error| format!("cannot list {}: {error}", dir.display()))?;
  found.remove(MANIFEST);
  let expected: BTreeSet<&str> = digests.keys().map(String::as_str).collect();
  if let Some(extra) = found.iter().find(|name| !expected.contains(name.as_str())) {
    return Err(format!("{extra} is not part of the index"));
  }
  if let Some(missing) = expected.iter().find(|name| !found.contains(**name)) {
    return Err(format!("{missing} is missing"));
  }
  let entries: Vec<(&String, &String)> = digests.iter().collect();
  let workers = thread::available_parallelism().map_or(1, NonZeroUsize::get);
  let chunk = entries.len().div_ceil(workers).max(1);
  thread::scope(|scope| {
    let checks: Vec<_> = entries
      .chunks(chunk)
      .map(|part| scope.spawn(move || part.iter().find_map(|(name, digest)| mismatch(dir, name, digest))))
      .collect();
    checks
      .into_iter()
      .find_map(|check| check.join().expect("a digest check does not panic"))
      .map_or(Ok(()), Err)
  })
}

/// Why `name` in `dir` does not have `digest`, if it does not.
fn mismatch(dir: &Path, name: &str, digest: &str) -> Option<String> {
  let path = name.split('/').fold(dir.to_path_buf(), |path, part| path.join(part));
  match fs::read(&path) {
    Ok(bytes) if of(&bytes) == digest => None,
    Ok(_) => Some(format!("{name} was changed")),
    Err(error) => Some(format!("cannot read {name}: {error}")),
  }
}

fn list(root: &Path, dir: &Path, found: &mut BTreeSet<String>) -> io::Result<()> {
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      list(root, &path, found)?;
    } else {
      let relative = path.strip_prefix(root).expect("listed under the root");
      let name: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
      found.insert(name.join("/"));
    }
  }
  Ok(())
}
