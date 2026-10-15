//! *Open file* on a changed file's head (`docs/design/code-viewer.md`, "Links to it"): the file in the code
//! viewer, in a new tab, at the version the change shows. A checkpointed change shows the file at the
//! checkpoint; a change not checkpointed yet, in the run's worktree. A deleted file is shown where it was
//! last: at the commit the diff starts from.

use orchester_core::{diff::FileDiff, id::RunId, revision::Revision};
use orchester_workspace::files::Version;

use crate::routes::{ChangeSource, FilesView};

/// The code viewer's view of `file`, a change of `source` from the commit `from`, of run `run` whose latest
/// checkpoint is `head`.
pub fn file_view(
  file: &FileDiff,
  source: ChangeSource,
  (run, from, head): (RunId, &Revision, Option<&Revision>),
) -> FilesView {
  match (file.new_path(), source) {
    (None, _) => FilesView::new(Some(file.path()), Version::Revision(from.clone())),
    (Some(path), ChangeSource::Worktree) => FilesView::new(Some(path), Version::Run(run)),
    (Some(path), ChangeSource::Checkpoint) => {
      let at = head.map_or(Version::Run(run), |head| Version::Revision(head.clone()));
      FilesView::new(Some(path), at)
    }
  }
}
