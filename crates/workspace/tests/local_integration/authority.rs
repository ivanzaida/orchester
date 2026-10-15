use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use orchester_core::integration::local::{DraftChoice, EditorCandidate, LocalError, LocalPhase, Placement};
use orchester_workspace::tasks::local_integration::{IntegrationError, LocalIntegration, LocalIntent, LocalOrigin};

use super::support::{Fixture, git};

#[test]
fn local_integration_scoped_draft_authority_is_rechecked_at_actual_execution_and_new_generation() {
  let fixture = Fixture::new(false);
  std::fs::write(fixture.root().join("task.txt"), "main conflict\n").expect("main");
  git(fixture.root(), &["commit", "-am", "main conflict"]);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("owned conflict");
  let generation = session.operation().version();
  let file = orchester_exec::local_git::LocalGit::new(
    &fixture.task_root,
    &fixture.task_root.join(".orchester/local-integration"),
  )
  .expect("private Git")
  .conflicts()
  .expect("file")
  .remove(0);
  let expected = orchester_exec::local_git::file_hash(&fixture.task_root, &file.path).expect("hash");
  drop(session);
  let allowed = Arc::new(AtomicBool::new(true));
  let actual = allowed.clone();
  let authorize = Arc::new(move || {
    if !actual.load(Ordering::Acquire) {
      return Err(LocalError::AuthorityChanged);
    }
    LocalOrigin::WindowAutomation.authorize(LocalIntent::Candidate)
  });
  let mut session = LocalIntegration::open_authorized_at(fixture.root(), fixture.run, Some(generation), authorize)
    .expect("authorized scope");
  let draft = EditorCandidate::new("raw\n<<<<<<< markers\n".into(), "../partial/".into(), Placement::Stage)
    .with_choice(DraftChoice::Current);
  let before = git(&fixture.task_root, &["ls-files", "--stage"]);
  allowed.store(false, Ordering::Release);
  assert!(matches!(
    session.save_editor_draft(&file, expected.as_ref(), &draft, generation),
    Err(IntegrationError::Git(orchester_exec::local_git::LocalGitError::State(
      LocalError::AuthorityChanged
    )))
      | Err(IntegrationError::State(LocalError::AuthorityChanged))
  ));
  assert_eq!(git(&fixture.task_root, &["ls-files", "--stage"]), before);
  assert_eq!(session.operation().version(), generation);
  drop(session);
  let mut resumed = fixture.open();
  assert!(
    resumed
      .editor_draft(&file, expected.as_ref())
      .expect("not saved")
      .is_none()
  );
  resumed
    .save_editor_draft(&file, expected.as_ref(), &draft, generation)
    .expect("fresh native scoped save");
  assert_eq!(
    resumed
      .editor_draft(&file, expected.as_ref())
      .expect("saved")
      .expect("draft"),
    draft
  );
  assert_eq!(resumed.operation().phase(), LocalPhase::TaskConflicts);
  assert!(resumed.operation().checked().is_none());
  drop(resumed);
  assert!(matches!(
    LocalIntegration::open_at(fixture.root(), fixture.run, Some(generation)),
    Err(IntegrationError::State(LocalError::Stale))
  ));
}

#[test]
fn local_integration_unfinished_choice_serialization_preserves_raw_data_without_applying_resolution() {
  let marker = "<<<<<<< raw\n=======\n>>>>>>> raw\n";
  let candidate =
    EditorCandidate::new(marker.into(), "../partial/".into(), Placement::Stage).with_choice(DraftChoice::KeepBoth);
  let encoded = serde_json::to_vec(&candidate).expect("private bytes");
  let resumed: EditorCandidate = serde_json::from_slice(&encoded).expect("raw draft after restart");
  assert_eq!(resumed, candidate);
  assert!(
    resumed.resolution().is_err(),
    "unfinished destination remains unfinished"
  );
  assert!(!format!("{resumed:?}").contains(marker));
}
