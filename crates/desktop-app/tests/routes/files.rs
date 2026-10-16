//! The code viewer's routes: a file at a version, on a range of its lines.

use orchester_app::routes::{FilesView, ProjectPage, Route, RouteError};
use orchester_core::{id::RunId, revision::Revision};
use orchester_workspace::files::{LineRange, Version};

use super::{AWKWARD, CONNECTION, PROJECT, at};

/// The code viewer at every version, with lines, and with paths it refuses but still reads back.
pub(super) fn files_routes() -> Vec<Route> {
  let run: RunId = CONNECTION.parse().expect("a run id");
  let revision = Revision::new("4d2b8e0c1f2a3b4c5d6e7f8091a2b3c4d5e6f708").expect("a commit");
  let lines = LineRange::new(441, 442);
  let mut all = vec![at(ProjectPage::Files(FilesView::default()))];
  for version in [
    Version::Checkout,
    Version::Base,
    Version::Run(run),
    Version::Revision(revision),
  ] {
    for path in [
      "crates/workspace/src/plan_limits.rs",
      AWKWARD,
      "../kontur/.env",
      "/etc/passwd",
      "a//b/",
      "%2F",
    ] {
      all.push(at(ProjectPage::Files(FilesView::new(Some(path), version.clone()))));
      all.push(at(ProjectPage::Files(
        FilesView::new(Some(path), version.clone()).with_line(lines),
      )));
    }
  }
  all
}

#[test]
fn files_paths_name_the_file_its_version_and_lines() {
  let root = format!("/project/{PROJECT}/files");
  let run: RunId = CONNECTION.parse().expect("a run id");
  let paths = [
    (FilesView::default(), root.clone()),
    (
      FilesView::new(Some("crates/workspace/src/plan_limits.rs"), Version::Checkout)
        .with_line(LineRange::new(441, 442)),
      format!("{root}/crates/workspace/src/plan_limits.rs?line=441-442"),
    ),
    (
      FilesView::new(Some("src/a b.rs"), Version::Run(run)).with_line(LineRange::new(7, 7)),
      format!("{root}/src/a%20b.rs?at=run%3A{CONNECTION}&line=7"),
    ),
    (
      FilesView::new(Some("README.md"), Version::Base),
      format!("{root}/README.md?at=base"),
    ),
    (
      FilesView::new(Some("/etc/passwd"), Version::Checkout),
      format!("{root}/%2Fetc/passwd"),
    ),
  ];
  for (view, path) in paths {
    assert_eq!(at(ProjectPage::Files(view)).path(), path);
  }
  assert!(matches!(
    Route::parse(&format!("{root}/README.md?at=worktree")),
    Err(RouteError::InvalidParam { .. })
  ));
  assert!(matches!(
    Route::parse(&format!("{root}/README.md?line=0")),
    Err(RouteError::InvalidParam { .. })
  ));
  assert!(matches!(
    Route::parse(&format!("{root}/README.md?file=x")),
    Err(RouteError::UnexpectedParam { .. })
  ));
}
