//! A run whose work was merged outside Orchester (git by hand, or a pull request pulled back): its local integration
//! that holds nothing reads as complete by itself and stops reserving the checkout; one that holds the person's
//! preserved originals stays for the person, who records the merge and gets the originals back. Only proven
//! containment of the whole work counts.

use std::fs;

use orchester_core::{
  integration::local::{LocalIntegrationStore, LocalPath, LocalPhase, ResultEvidenceKind},
  store::MergeStore,
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::{IntegrationError, external_integration};

use super::support::{Fixture, git};

/// The run's review, paused before it held anything, and the run's branch then merged into main by hand.
fn paused_then_merged_outside(fixture: &Fixture) -> String {
  let mut session = fixture.open();
  session.pause().expect("pause the review");
  drop(session);
  git(fixture.root(), &["merge", "--ff-only", "task"]);
  git(fixture.root(), &["rev-parse", "HEAD"])
}

#[test]
fn local_integration_merged_outside_completes_a_paused_review_that_holds_nothing() {
  let fixture = Fixture::new(false);
  let head = paused_then_merged_outside(&fixture);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("contained");
  assert_eq!(evidence.kind, ResultEvidenceKind::CompleteAncestry);
  let db = ProjectDb::open(fixture.root()).expect("db");
  let settled = db.local_operation(fixture.run).expect("read").expect("the record");
  assert_eq!(settled.phase(), LocalPhase::Complete, "the review reads as complete");
  assert!(!settled.reserves_checkout(), "the project's other merges are not held");
  assert!(settled.merged().is_none(), "no Orchester merge is invented");
  assert_eq!(settled.merged_outside(), Some(head.as_str()));
  assert!(
    external_integration(fixture.root(), fixture.run)
      .expect("evidence again")
      .is_some(),
    "the tab still shows the merge made without Orchester"
  );
}

#[test]
fn local_integration_merged_outside_keeps_preserved_originals_for_the_person_to_finish() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "base\nthe person's edit\n").expect("edit");
  let mut session = fixture.open();
  session.preserve("Before the merge").expect("preserve");
  drop(session);
  git(fixture.root(), &["merge", "--ff-only", "task"]);
  let head = git(fixture.root(), &["rev-parse", "HEAD"]);
  assert!(
    external_integration(fixture.root(), fixture.run)
      .expect("evidence")
      .is_some()
  );
  let db = ProjectDb::open(fixture.root()).expect("db");
  let kept = db.local_operation(fixture.run).expect("read").expect("the record");
  assert_eq!(
    kept.phase(),
    LocalPhase::Prepared,
    "the person's originals are not settled away"
  );
  assert!(kept.reserves_checkout());
  let mut session = fixture.open();
  session.record_merged_outside().expect("record the merge found in git");
  assert_eq!(session.operation().phase(), LocalPhase::MergedAwaitingRestore);
  assert_eq!(session.operation().merged(), Some(head.as_str()));
  assert_eq!(session.operation().merged_outside(), Some(head.as_str()));
  session.pause().expect("pause before restoring");
  session.reconcile().expect("reconcile the recorded merge");
  assert_eq!(session.operation().phase(), LocalPhase::MergedAwaitingRestore);
  assert!(
    db.run_merge(fixture.run).expect("merge record").is_none(),
    "a merge made without Orchester never becomes an Orchester merge record"
  );
  session.restore().expect("put the originals back");
  assert_eq!(
    fs::read_to_string(fixture.root().join("local.txt")).expect("the person's file"),
    "base\nthe person's edit\n"
  );
  session.remove_preservation().expect("remove the kept originals");
  assert_eq!(session.operation().phase(), LocalPhase::Complete);
  assert!(!session.operation().reserves_checkout());
  assert_eq!(
    git(fixture.root(), &["rev-parse", "HEAD"]),
    head,
    "nothing was merged again"
  );
}

#[test]
fn local_integration_partial_containment_settles_nothing_and_cannot_be_recorded() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.pause().expect("pause the review");
  drop(session);
  let first = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  fs::write(fixture.task_root.join("remaining.txt"), "not delivered\n").expect("edit");
  git(&fixture.task_root, &["add", "remaining.txt"]);
  git(&fixture.task_root, &["commit", "-m", "second result"]);
  git(fixture.root(), &["merge", "--ff-only", &first]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("partial");
  assert_eq!(evidence.kind, ResultEvidenceKind::PartialAncestry);
  let db = ProjectDb::open(fixture.root()).expect("db");
  let kept = db.local_operation(fixture.run).expect("read").expect("the record");
  assert_eq!(
    kept.phase(),
    LocalPhase::Paused,
    "partial ancestry still asks the person"
  );
  assert!(kept.merged_outside().is_none());
  let mut session = fixture.open();
  assert!(matches!(
    session.record_merged_outside(),
    Err(IntegrationError::State(_))
  ));
  assert!(session.operation().merged().is_none());
}

#[test]
fn local_integration_partial_containment_with_a_held_commit_reconciles_then_cancels() {
  let fixture = Fixture::new(false);
  fs::write(
    fixture.root().join("local.txt"),
    "base
the person's commit
",
  )
  .expect("edit");
  git(fixture.root(), &["add", "local.txt"]);
  let mut session = fixture.open();
  session.refresh().expect("review the edit");
  let path = LocalPath::new("local.txt").expect("path");
  let tree = session.selected_commit_tree(std::slice::from_ref(&path)).expect("tree");
  session.commit(&[path], &tree, "the person's commit").expect("commit");
  assert!(session.operation().reserves_checkout(), "the commit is held");
  drop(session);
  let first = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  fs::write(
    fixture.task_root.join("remaining.txt"),
    "not delivered
",
  )
  .expect("edit");
  git(&fixture.task_root, &["add", "remaining.txt"]);
  git(&fixture.task_root, &["commit", "-m", "second result"]);
  git(fixture.root(), &["merge", "--no-edit", &first]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("partial");
  assert_eq!(evidence.kind, ResultEvidenceKind::PartialAncestry);
  let mut session = fixture.open();
  assert!(
    session.cancel_preservation().is_err(),
    "the checkout moved since the review"
  );
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("reconcile reviews the moved checkout");
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  session
    .cancel_preservation()
    .expect("then the integration is cancelled");
  assert_eq!(session.operation().phase(), LocalPhase::Canceled);
  assert!(!session.operation().reserves_checkout());
}
