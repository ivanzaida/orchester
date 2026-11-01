//! A parallel step's worktree starts with a warm build (ORC-544). A step that starts beside others works in a worktree
//! of its own, which has no `target/`, so its first cargo build would be cold. When the run's worktree has a
//! `target/` on the same volume, the step's worktree gets it as a tree of hard links ([`files`] says which files are
//! copied or left out instead, since a build would write them in place), and its checked-out files and folders take
//! the times of the run's same ones ([`sources`]), so cargo finds the run's artifacts fresh and builds only what
//! changed. A build in the step's worktree never changes a file of the run's.
//!
//! Only the profile folders cargo builds into are seeded (`debug/`, `release/`, `<target triple>/debug/`, each known by
//! its `.fingerprint/`); other folders, such as `target/tmp/` that tests write into, are not.
//!
//! Removing the step's worktree removes its links: a file's data stays while the run's link to it does. Disk usage is
//! read as free space, and linked data takes none twice.

mod files;
mod sources;
mod times;

use std::{
  fs, io,
  path::Path,
  time::{Duration, Instant},
};

use self::files::Seeding;

const TARGET: &str = "target";
const FINGERPRINTS: &str = ".fingerprint";

/// What seeding a step's `target/` did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Seeded {
  /// Files linked to the run's, and their bytes.
  pub linked: u64,
  pub linked_bytes: u64,
  /// Files copied, since a build writes them in place, and their bytes.
  pub copied: u64,
  pub copied_bytes: u64,
  /// Checked-out files and folders that took the time of the run's same one.
  pub aligned: u64,
  pub took: Duration,
}

/// Seeds `step_worktree`'s `target/` from `run_worktree`'s; `None` when the step starts as before: the run has no
/// `target/` with a profile folder, the step has one already, or the two are not on one volume (the first link
/// fails). A seeding that fails halfway is removed again.
pub fn seed(run_worktree: &Path, step_worktree: &Path) -> Option<Seeded> {
  let started = Instant::now();
  let from = run_worktree.join(TARGET);
  let to = step_worktree.join(TARGET);
  if !from.is_dir() || to.exists() {
    return None;
  }
  let mut seeded = Seeded::default();
  if let Err(error) = seeded.target(&from, &to) {
    tracing::info!(step = %step_worktree.display(), %error, "the run's build output is not seeded");
    discard(&to);
    return None;
  }
  if seeded.linked == 0 {
    discard(&to);
    return None;
  }
  seeded.aligned = sources::align(run_worktree, step_worktree);
  seeded.took = started.elapsed();
  tracing::info!(
    step = %step_worktree.display(),
    linked = seeded.linked,
    linked_bytes = seeded.linked_bytes,
    copied = seeded.copied,
    copied_bytes = seeded.copied_bytes,
    aligned = seeded.aligned,
    took_ms = seeded.took.as_millis(),
    "seeded the step's build output from the run's"
  );
  Some(seeded)
}

/// Removes a seeding that did not finish. Its links only go; the run's files stay.
fn discard(target: &Path) {
  if let Err(error) = fs::remove_dir_all(target)
    && error.kind() != io::ErrorKind::NotFound
  {
    tracing::warn!(target = %target.display(), %error, "a half-seeded build output is left");
  }
}

/// Whether `folder` is a profile folder cargo builds into.
fn is_profile(folder: &Path) -> bool {
  folder.join(FINGERPRINTS).is_dir()
}

impl Seeded {
  /// The profile folders of `from`, a `target/`, and of the target triple folders in it, seeded into `to`.
  fn target(&mut self, from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
      let entry = entry?;
      if !entry.file_type()?.is_dir() {
        continue;
      }
      let folder = entry.path();
      if is_profile(&folder) {
        self.profile(&folder, &to.join(entry.file_name()))?;
        continue;
      }
      for inner in fs::read_dir(&folder)? {
        let inner = inner?;
        if inner.file_type()?.is_dir() && is_profile(&inner.path()) {
          let triple = to.join(entry.file_name());
          if !triple.is_dir() {
            fs::create_dir(&triple)?;
          }
          self.profile(&inner.path(), &triple.join(inner.file_name()))?;
        }
      }
    }
    Ok(())
  }

  /// Every file under the profile folder `from`, linked, copied into `to` or left out as [`files::seeding`] says.
  /// Links are not followed.
  fn profile(&mut self, from: &Path, to: &Path) -> io::Result<()> {
    let mut pending = vec![(from.to_path_buf(), to.to_path_buf())];
    while let Some((source, destination)) = pending.pop() {
      fs::create_dir(&destination)?;
      for entry in fs::read_dir(&source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = entry.path();
        let into = destination.join(entry.file_name());
        if kind.is_dir() {
          if !files::leaves_folder(&entry.file_name()) {
            pending.push((path, into));
          }
        } else if kind.is_file() {
          let relative = path.strip_prefix(from).unwrap_or(&path);
          match files::seeding(&path, relative) {
            Seeding::Link => self.link(&path, &into)?,
            Seeding::Copy => self.copy(&path, &into)?,
            Seeding::Leave => {}
          }
        }
      }
    }
    Ok(())
  }

  /// `from` linked to `to`; copied when the link fails after others were made (a file another program holds). The
  /// first link failing means another volume, so the seeding stops.
  fn link(&mut self, from: &Path, to: &Path) -> io::Result<()> {
    match fs::hard_link(from, to) {
      Ok(()) => {
        self.linked += 1;
        self.linked_bytes = self.linked_bytes.saturating_add(fs::symlink_metadata(to)?.len());
        Ok(())
      }
      Err(error) if self.linked == 0 => Err(error),
      Err(_) => self.copy(from, to),
    }
  }

  /// `from` copied to `to` with its time: a fingerprint newer than the run's would take stale artifacts for fresh, and
  /// a proc macro newer than its users would rebuild them.
  fn copy(&mut self, from: &Path, to: &Path) -> io::Result<()> {
    let bytes = fs::copy(from, to)?;
    times::set_modified(to, fs::metadata(from)?.modified()?)?;
    self.copied += 1;
    self.copied_bytes = self.copied_bytes.saturating_add(bytes);
    Ok(())
  }
}
