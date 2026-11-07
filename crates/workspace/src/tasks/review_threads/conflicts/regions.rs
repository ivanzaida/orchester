//! Where a run's changes meet the base branch's in the files that conflict: the run's changed lines that touch lines
//! the base branch changed since the run's work started, and what the base has there now. Both are read from the two
//! diffs of the file against the revision the run's own work starts from, so the lines of one are placed against the
//! other's by their old line numbers; changes that touch (adjacent lines, or insertions at one place) conflict as git
//! merges them.

use std::path::Path;

use orchester_core::{
  diff::{Diff, FileDiff, LineKind, git::read_path_diff},
  pull_request::feedback::DiffSide,
  revision::Revision,
};
use orchester_exec::programs::Programs;

use crate::tasks::changes::GIT;

/// A line of the base branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseLine {
  pub number: u32,
  pub text: String,
}

/// Lines of the run's file that conflict with the base branch's, and what the base has there now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictRegion {
  pub path: String,
  /// The side of the run's diff its lines are numbered on: the new file's, or the old one's when the run only removed
  /// lines here.
  pub side: DiffSide,
  /// The first and last line of the run's change here.
  pub lines: (u32, u32),
  /// What the base branch has in these lines now; none when it removed them.
  pub base_lines: Vec<BaseLine>,
}

/// A run of changed lines of a file's diff.
struct Change {
  /// The first old line it changes, or the old line a pure insertion comes before.
  from: u32,
  removed: Vec<u32>,
  added: Vec<BaseLine>,
}

impl Change {
  const fn new(from: u32) -> Self {
    Self {
      from,
      removed: Vec::new(),
      added: Vec::new(),
    }
  }

  /// The old line after the last one it changes.
  fn until(&self) -> u32 {
    self.from + u32::try_from(self.removed.len()).unwrap_or(u32::MAX)
  }

  fn touches(&self, other: &Self) -> bool {
    self.from <= other.until() && other.from <= self.until()
  }
}

/// The regions of every file in `files` that `run_diff` changes and the base branch changed too between `from` and
/// `tip`. A file git cannot show the base's changes of is left out and logged.
pub(super) fn read(
  (root, programs): (&Path, &Programs),
  (from, tip): (&Revision, &Revision),
  (run_diff, files): (&Diff, &[String]),
) -> Vec<ConflictRegion> {
  let mut regions = Vec::new();
  for file in run_diff
    .files()
    .iter()
    .filter(|file| files.iter().any(|path| path == file.path()))
  {
    match read_path_diff(&programs.runner(), Path::new(GIT), root, (from, tip), file.path()) {
      Ok(base) => regions.extend(of_file(file, &base)),
      Err(error) => tracing::warn!(%error, path = file.path(), "cannot read what the base branch changed in a file"),
    }
  }
  regions
}

/// The regions of `file`, the run's diff of it, against `base`, the base branch's diff of it.
fn of_file(file: &FileDiff, base: &Diff) -> Vec<ConflictRegion> {
  let base_changes: Vec<Change> = base.files().iter().flat_map(changes).collect();
  changes(file)
    .into_iter()
    .filter_map(|change| {
      let touching: Vec<&Change> = base_changes.iter().filter(|other| change.touches(other)).collect();
      if touching.is_empty() {
        return None;
      }
      let (side, lines) = match (change.added.first(), change.added.last()) {
        (Some(first), Some(last)) => (DiffSide::New, (first.number, last.number)),
        _ => (
          DiffSide::Old,
          (change.from, change.removed.last().copied().unwrap_or(change.from)),
        ),
      };
      Some(ConflictRegion {
        path: file.path().to_owned(),
        side,
        lines,
        base_lines: touching.iter().flat_map(|other| other.added.clone()).collect(),
      })
    })
    .collect()
}

/// The runs of changed lines of `file`, in order.
fn changes(file: &FileDiff) -> Vec<Change> {
  let mut found = Vec::new();
  for hunk in file.hunks() {
    // `-0,0` places an insertion before the first line.
    let mut next_old = if hunk.old_count() == 0 {
      hunk.old_start() + 1
    } else {
      hunk.old_start()
    };
    let mut open: Option<Change> = None;
    for line in hunk.lines() {
      match line.kind() {
        LineKind::Context => {
          found.extend(open.take());
          next_old = line.old_number().map_or(next_old, |number| number + 1);
        }
        LineKind::Removed => {
          let number = line.old_number().unwrap_or(next_old);
          open.get_or_insert_with(|| Change::new(next_old)).removed.push(number);
          next_old = number + 1;
        }
        LineKind::Added => {
          if let Some(number) = line.new_number() {
            open.get_or_insert_with(|| Change::new(next_old)).added.push(BaseLine {
              number,
              text: line.text().to_owned(),
            });
          }
        }
      }
    }
    found.extend(open);
  }
  found
}
