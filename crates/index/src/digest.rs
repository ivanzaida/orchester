//! Content digests of an index's files, kept in its `manifest.json`. An index is shared by the runs
//! of its revision and its outline files by later indexes, so a file an agent changed would spread:
//! an index is checked against its digests before it is reused, handed to an agent or built on,
//! and one that does not match is built again instead. Reading every file of a large index takes
//! seconds, so this process remembers the size and change time of each file of an index it found
//! intact: while none of them changed, the index is not read again.

use std::{
  collections::{BTreeMap, HashMap},
  fs,
  io::{self, Write},
  num::NonZeroUsize,
  path::{Path, PathBuf},
  sync::LazyLock,
  thread,
  time::SystemTime,
};

use parking_lot::Mutex;
use sha2::{Digest, Sha256};

/// The SHA-256 of each file of an index but its manifest, by its path in the index (`tree.txt`,
/// `outline/src/lib.rs.md`).
pub(crate) type Digests = BTreeMap<String, String>;

const MANIFEST: &str = "manifest.json";

/// The files of an index folder, the manifest included, by path in the index: size and change time.
type Stats = BTreeMap<String, (u64, SystemTime)>;

/// The indexes this process found intact, by folder, as they were then.
static INTACT: LazyLock<Mutex<HashMap<PathBuf, Stats>>> = LazyLock::new(Mutex::default);

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
/// when it does not. The files are hashed on a thread per core, unless this process found them
/// intact before and none changed size or change time since.
pub(crate) fn verify(dir: &Path, digests: &Digests) -> Result<(), String> {
  let mut stats = Stats::new();
  list(dir, dir, &mut stats).map_err(|error| format!("cannot list {}: {error}", dir.display()))?;
  if INTACT.lock().get(dir) == Some(&stats) {
    return Ok(());
  }
  if let Some(extra) = stats
    .keys()
    .find(|name| name.as_str() != MANIFEST && !digests.contains_key(name.as_str()))
  {
    return Err(format!("{extra} is not part of the index"));
  }
  if let Some(missing) = digests.keys().find(|name| !stats.contains_key(name.as_str())) {
    return Err(format!("{missing} is missing"));
  }
  hash_all(dir, digests)?;
  INTACT.lock().insert(dir.to_path_buf(), stats);
  Ok(())
}

/// Whether every file of `digests` in `dir` has its digest.
fn hash_all(dir: &Path, digests: &Digests) -> Result<(), String> {
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

fn list(root: &Path, dir: &Path, found: &mut Stats) -> io::Result<()> {
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    let metadata = entry.metadata()?;
    if metadata.is_dir() {
      list(root, &path, found)?;
    } else {
      let relative = path.strip_prefix(root).expect("listed under the root");
      let name: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
      found.insert(name.join("/"), (metadata.len(), metadata.modified()?));
    }
  }
  Ok(())
}
