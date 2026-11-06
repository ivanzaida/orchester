//! What earlier verified reads of a checkout proved, so a verified read hashes only the tracked files that may have
//! changed since. A proof is a tracked path's index entry with its working file's stamp: size, modification and change
//! time, identity and type, read without opening the content. The change time is the operating system's own (`ctime`
//! on Unix, NTFS's `ChangeTime` on Windows): every write and every timestamp change sets it to the current time and no
//! ordinary call sets it back, so an edit that kept the size and restored the modification time still changes the
//! stamp. A proof is kept only for a file that last changed well before the read began, so a write in the same clock
//! tick as the read cannot share the stamp the read saw.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::{
  collections::BTreeMap,
  io::Write as _,
  path::{Path, PathBuf},
  time::{SystemTime, UNIX_EPOCH},
};

use orchester_core::integration::local::{IndexEntry, LocalPath};
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

use super::{LocalGitError, files::held_directory};

/// Kept in the checkout's own Orchester folder, beside the local integration's scratch folders.
const FOLDER: [&str; 2] = [".orchester", "local-integration"];
const FILE: &str = "verified-stamps";
const HEADER: &[u8] = b"orchester-verified-stamps 1\n";
/// How long before a read began a file must have last changed for its stamp to prove it, in nanoseconds: more than
/// any file system's timestamp granularity (FAT's two seconds).
const SETTLED: i128 = 3_000_000_000;
/// A submodule's working state is its own HEAD, which no stamp of its folder tells.
const GITLINK: u32 = 0o160_000;

/// A working file's stamp, read without opening its content. Times are nanoseconds since the Unix epoch.
struct Stamp {
  size: u64,
  modified: i128,
  changed: i128,
  identity: (u64, u64),
  kind: u32,
}

/// A path's index entry with its working file's stamp, as one key.
struct Seen {
  key: String,
  /// The file last changed long enough before the read began that no later write can share its stamp.
  settled: bool,
}

/// The stamps of one verified read, with what earlier verified reads proved: each path's key, as text. A path or
/// key in the saved proofs that no entry has now simply proves nothing.
pub(super) struct Proof {
  folder: PathBuf,
  proven: BTreeMap<String, String>,
  seen: BTreeMap<LocalPath, Seen>,
}

impl Proof {
  /// Stamps the working file of every stage-0 entry of the checkout at `root`, before Git reads any of them.
  pub fn read(root: &Path, entries: &BTreeMap<LocalPath, IndexEntry>) -> Result<Self, LocalGitError> {
    let started = nanoseconds(SystemTime::now());
    let folder = held_directory(root, &FOLDER)?.path().to_owned();
    let proven = load(&folder.join(FILE));
    let seen = entries
      .iter()
      .filter(|(_, entry)| entry.mode != GITLINK)
      .filter_map(|(path, entry)| {
        let stamp = platform::stamp(&location(root, path))?;
        let seen = Seen {
          key: key(entry, &stamp),
          settled: stamp.changed + SETTLED < started,
        };
        Some((path.clone(), seen))
      })
      .collect();
    Ok(Self { folder, proven, seen })
  }

  /// The paths whose index entry and working file are still exactly as a verified read proved them: Git need not
  /// hash them again.
  pub fn unchanged(&self) -> Vec<&LocalPath> {
    self
      .seen
      .iter()
      .filter(|(path, seen)| self.proves(path, seen))
      .map(|(path, _)| path)
      .collect()
  }

  /// Keeps this read's proofs for the next one: the unchanged paths, and each settled path that `holds_entry` says
  /// Git found holding its index entry's content.
  pub fn keep(self, holds_entry: impl Fn(&LocalPath) -> bool) {
    let proven = self
      .seen
      .iter()
      .filter(|(path, seen)| self.proves(path, seen) || (seen.settled && holds_entry(path)))
      .map(|(path, seen)| (path.as_str().to_owned(), seen.key.clone()))
      .collect::<BTreeMap<_, _>>();
    if proven == self.proven {
      return;
    }
    // Proofs that cannot be written only leave the next verified read more to hash.
    if let Err(error) = save(&self.folder, &proven) {
      tracing::warn!(%error, "cannot keep the stamps a verified checkout read proved");
    }
  }

  fn proves(&self, path: &LocalPath, seen: &Seen) -> bool {
    self.proven.get(path.as_str()) == Some(&seen.key)
  }
}

fn key(entry: &IndexEntry, stamp: &Stamp) -> String {
  let Stamp {
    size,
    modified,
    changed,
    identity: (volume, file),
    kind,
  } = stamp;
  format!(
    "{:o} {} {size} {modified} {changed} {volume} {file} {kind}",
    entry.mode, entry.object
  )
}

fn location(root: &Path, path: &LocalPath) -> PathBuf {
  path
    .as_str()
    .split('/')
    .fold(root.to_owned(), |location, part| location.join(part))
}

fn nanoseconds(time: SystemTime) -> i128 {
  match time.duration_since(UNIX_EPOCH) {
    Ok(after) => i128::try_from(after.as_nanos()).unwrap_or(i128::MAX),
    Err(before) => -i128::try_from(before.duration().as_nanos()).unwrap_or(i128::MAX),
  }
}

/// The proofs saved at `file`; none when there are none or they cannot be read, which only means more to hash.
fn load(file: &Path) -> BTreeMap<String, String> {
  let Ok(bytes) = std::fs::read(file) else {
    return BTreeMap::new();
  };
  parse(&bytes).unwrap_or_default()
}

fn parse(bytes: &[u8]) -> Option<BTreeMap<String, String>> {
  let text = std::str::from_utf8(bytes.strip_prefix(HEADER)?).ok()?;
  let mut fields = text.split('\0');
  let mut proven = BTreeMap::new();
  while let Some(path) = fields.next().filter(|path| !path.is_empty()) {
    proven.insert(path.to_owned(), fields.next()?.to_owned());
  }
  Some(proven)
}

/// Replaces the saved proofs at once, so a reader sees the earlier ones or these.
fn save(folder: &Path, proven: &BTreeMap<String, String>) -> std::io::Result<()> {
  let mut bytes = HEADER.to_vec();
  for (path, key) in proven {
    bytes.extend_from_slice(path.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(key.as_bytes());
    bytes.push(0);
  }
  let mut file = tempfile::Builder::new().prefix("stamps-").tempfile_in(folder)?;
  file.write_all(&bytes)?;
  file.persist(folder.join(FILE)).map_err(|error| error.error)?;
  Ok(())
}
