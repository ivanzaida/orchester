//! The earlier index an incremental build starts from: of the indexes with the current format, the
//! one whose revision differs from the new one in the fewest paths, provided no one is building or
//! removing it and its files still match their digests. Its shared lock is held while the build
//! reads it: runs reusing it and other builds reading it go on meanwhile, a removal or rebuild waits.

use orchester_core::revision::Revision;

use super::{
  CodebaseIndexes,
  locks::{self, RevisionLock},
};
use crate::{build::Base, digest, git::Repository, manifest::Manifest};

impl CodebaseIndexes {
  /// The base of an incremental build of `revision`, with its lock; `None` builds from scratch.
  pub(super) fn base(&self, revision: &Revision) -> Option<(Base, RevisionLock)> {
    let git = Repository::new(&self.repo);
    let mut candidates: Vec<_> = self
      .revisions()
      .into_iter()
      .filter(|existing| existing != revision)
      .filter_map(|existing| match git.changed_paths(&existing, revision) {
        Ok(changed) => Some((existing, changed)),
        Err(error) => {
          tracing::warn!(%error, from = %existing, "cannot compare with an existing codebase index");
          None
        }
      })
      .collect();
    candidates.sort_by(|(left, left_changed), (right, right_changed)| {
      (left_changed.len(), left).cmp(&(right_changed.len(), right))
    });
    candidates.into_iter().find_map(|(existing, changed)| {
      // A revision someone builds or removes now, or whose lock cannot be taken, is passed over.
      let lock = locks::try_lock_shared(&self.root, &existing).ok().flatten()?;
      let dir = self.root.join(existing.as_str());
      let manifest = Manifest::read_current(&dir)?;
      if let Err(problem) = digest::verify(&dir, &manifest.digests) {
        tracing::warn!(from = %existing, %problem, "a changed codebase index is no base to build on");
        return None;
      }
      let base = Base {
        dir,
        revision: existing,
        changed,
        digests: manifest.digests,
      };
      Some((base, lock))
    })
  }
}
