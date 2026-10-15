use std::fs;

use orchester_core::integration::local::{EditorCandidate, LocalError, LocalPhase, Placement, Resolution};
use orchester_exec::local_git::file_hash;
use orchester_workspace::tasks::local_integration::IntegrationError;

use super::support::{Fixture, git};

#[test]
fn local_integration_unfinished_markers_destination_and_staging_survive_pause_without_resolving() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("task.txt"), "main conflict\n").expect("edit");
  git(fixture.root(), &["commit", "-am", "main"]);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("conflicts");
  let scratch = fixture.task_root.join(".orchester/local-integration");
  let task = orchester_exec::local_git::LocalGit::new(&fixture.task_root, &scratch).expect("task");
  let file = task.conflicts().expect("files").remove(0);
  let expected = file_hash(&fixture.task_root, &file.path).expect("hash");
  let before = task.snapshot().expect("before");
  let marker_draft = "<<<<<<< mine\nunfinished\n=======\nother\n>>>>>>> theirs\n";
  let raw = EditorCandidate::new(marker_draft.into(), "../partial/".into(), Placement::Stage);
  session
    .save_editor_draft(&file, expected.as_ref(), &raw, session.operation().version())
    .expect("save draft");
  session.pause().expect("pause");
  assert_eq!(task.snapshot().expect("unchanged"), before);
  assert!(session.operation().checked().is_none());
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("resume owned conflict");
  assert_eq!(session.operation().phase(), LocalPhase::TaskConflicts);
  assert_eq!(
    session.editor_draft(&file, expected.as_ref()).expect("draft"),
    Some(raw)
  );
  assert!(matches!(
    session.continue_task(),
    Err(
      IntegrationError::State(LocalError::Unresolved)
        | IntegrationError::Git(orchester_exec::local_git::LocalGitError::State(LocalError::Unresolved))
    )
  ));
  assert_eq!(task.snapshot().expect("still unresolved"), before);
  session
    .save_task_resolution(
      &file,
      expected.as_ref(),
      &Resolution::Text("explicit resolved result\n".into()),
    )
    .expect("resolve explicitly");
  assert!(task.conflicts().expect("resolved").is_empty());
  session.continue_task().expect("fresh Check");
  assert_eq!(session.operation().phase(), LocalPhase::AwaitingApproval);
  assert_eq!(
    git(&fixture.task_root, &["show", "HEAD:task.txt"]),
    "explicit resolved result"
  );
}

#[test]
fn local_integration_approval_generation_cannot_reuse_identical_same_base_result() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("check");
  let exact = session.operation().checked().expect("result").clone();
  let generation = session.operation().version();
  let mut replacement = session.operation().clone();
  replacement.advance_version();
  drop(session);
  assert!(
    orchester_core::integration::local::LocalIntegrationStore::save_local_operation(
      &mut orchester_store::ProjectDb::open(fixture.root()).expect("db"),
      &replacement,
      Some(generation)
    )
    .expect("new durable generation")
  );
  let mut session = fixture.open();
  assert_eq!(session.operation().checked(), Some(&exact));
  assert!(session.operation().version() > generation);
  let head = git(fixture.root(), &["rev-parse", "HEAD"]);
  assert!(matches!(
    session.approve_and_merge_at(&exact, generation),
    Err(IntegrationError::State(LocalError::Stale))
  ));
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), head);
  assert!(session.operation().merged().is_none());
}
