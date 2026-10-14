//! What each reviewed snapshot of a changed path holds, as the local review lists it: the lines a staged or
//! working-copy diff adds and removes, or that it is binary, and a new file's size and kind.

use std::collections::BTreeMap;

use orchester_core::integration::local::{Checkout, LocalPath};
use orchester_exec::local_git::LocalGit;

use super::IntegrationError;

/// Where Git looks for a NUL byte to call content binary (`buffer_is_binary`).
const BINARY_PROBE: usize = 8000;

/// One reviewed snapshot of a changed path: HEAD → index, index → working copy, or an untracked file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Snapshot {
  Staged,
  Unstaged,
  Untracked,
}

/// The snapshot of `path` whose diff the review shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownSnapshot {
  pub path: LocalPath,
  pub snapshot: Snapshot,
}

/// How much a snapshot changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotSize {
  /// A text diff adding and removing these many lines.
  Lines { added: u64, removed: u64 },
  /// A new text file.
  NewText,
  /// Binary content: no text diff; a new file's size in bytes when it was read.
  Binary(Option<u64>),
}

/// The size of every snapshot of the reviewed checkout that Git or the file could report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SnapshotStats(BTreeMap<(LocalPath, Snapshot), SnapshotSize>);

impl SnapshotStats {
  pub fn of(&self, path: &LocalPath, snapshot: Snapshot) -> Option<SnapshotSize> {
    self.0.get(&(path.clone(), snapshot)).copied()
  }

  /// Reads the sizes from `main` for the paths of `checkout`; a path Git reports that the checkout does not
  /// list is left out.
  pub(super) fn read(main: &LocalGit, checkout: &Checkout) -> Result<Self, IntegrationError> {
    let mut stats = BTreeMap::new();
    for (snapshot, staged) in [(Snapshot::Staged, true), (Snapshot::Unstaged, false)] {
      for (path, size) in numstat(main, staged)? {
        if checkout.files.iter().any(|file| file.path == path) {
          stats.insert((path, snapshot), size);
        }
      }
    }
    for file in checkout
      .files
      .iter()
      .filter(|file| file.head.is_none() && file.staged.is_none())
    {
      if let Some(bytes) = main.file(&file.path)? {
        stats.insert((file.path.clone(), Snapshot::Untracked), new_file(&bytes));
      }
    }
    Ok(Self(stats))
  }
}

fn numstat(main: &LocalGit, staged: bool) -> Result<Vec<(LocalPath, SnapshotSize)>, IntegrationError> {
  let mut args = vec![
    "diff",
    "--no-color",
    "--no-ext-diff",
    "--no-textconv",
    "--no-renames",
    "--numstat",
    "-z",
  ];
  if staged {
    args.push("--cached");
  }
  args.push("--");
  let output = main.text(&args)?;
  Ok(output.split('\0').filter_map(numstat_entry).collect())
}

/// One `-z` numstat entry, `added<TAB>removed<TAB>path`; `-` for both counts of binary content. A path the
/// checkout model refuses is not one the review lists, so it gets no size rather than failing the read.
fn numstat_entry(entry: &str) -> Option<(LocalPath, SnapshotSize)> {
  let mut parts = entry.splitn(3, '\t');
  let (added, removed, path) = (parts.next()?, parts.next()?, parts.next()?);
  let path = LocalPath::new(path).ok()?;
  let size = match (added.trim().parse(), removed.trim().parse()) {
    (Ok(added), Ok(removed)) => SnapshotSize::Lines { added, removed },
    _ => SnapshotSize::Binary(None),
  };
  Some((path, size))
}

fn new_file(bytes: &[u8]) -> SnapshotSize {
  if bytes[..bytes.len().min(BINARY_PROBE)].contains(&0) {
    // A length beyond u64 cannot be held in memory; the size is then simply not shown.
    SnapshotSize::Binary(u64::try_from(bytes.len()).ok())
  } else {
    SnapshotSize::NewText
  }
}
