//! A restored file the person changed after restoration does not block cleanup forever: the review names it, and
//! cleanup the person confirms with it listed removes only the retained originals and leaves the edit.
use std::fs;

use orchester_core::integration::local::{LocalError, LocalPath, LocalPhase, LocalRecoveryStore, RecoveryLayer};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::{IntegrationError, read};

use super::{privacy::Vault, support::Fixture};

#[test]
fn local_cleanup_leaves_the_persons_edits_since_restoration_and_names_them() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "retained original\n").expect("edit");
  let mut session = fixture.open();
  session.preserve("Before cleanup").expect("preserve");
  session.sync().expect("check");
  let checked = session.operation().checked().expect("checked").clone();
  session.approve_and_merge(&checked).expect("merge");
  session.restore().expect("restore");
  assert_eq!(session.operation().phase(), LocalPhase::RestorationVerified);
  fs::write(
    fixture.root().join("local.txt"),
    "the person's edit after restoration\n",
  )
  .expect("person edits");
  let path = LocalPath::new("local.txt").expect("path");
  let view = read(fixture.root(), fixture.run, None, &Vault(None)).expect("review");
  assert_eq!(
    view.changed_since_restoration,
    vec![path.clone()],
    "the review names the edited file"
  );
  assert!(matches!(
    session.remove_preservation(),
    Err(IntegrationError::State(LocalError::Stale))
  ));
  session
    .remove_preservation_accepting(std::slice::from_ref(&path))
    .expect("cleanup with the edit listed");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("file"),
    b"the person's edit after restoration\n",
    "the edit stays"
  );
  assert!(!session.operation().preservation().expect("record").retained);
  assert_eq!(
    ProjectDb::open(fixture.root())
      .expect("db")
      .local_content(fixture.run, &path, RecoveryLayer::Working)
      .expect("content"),
    None,
    "only the retained originals are removed"
  );
}
