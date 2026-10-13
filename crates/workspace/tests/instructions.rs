//! Checked editor writes preserve the shared instruction lifecycle and reject outside changes.
use std::{collections::BTreeSet, path::PathBuf};

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::{AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, PromptText},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  agents::data::VersionState,
  instructions::{data::ProjectInstructions, editor},
};

fn project() -> tempfile::TempDir {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/instructions/workspace");
  std::fs::create_dir_all(&scratch).expect("valid instruction fixture");
  let root = tempfile::tempdir_in(scratch).expect("valid instruction fixture");
  std::fs::create_dir(root.path().join(".git")).expect("valid instruction fixture");
  ProjectDb::create(
    root.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("TST").expect("valid instruction fixture"),
      Name::new("Test").expect("valid instruction fixture"),
    ),
  )
  .expect("valid instruction fixture");
  root
}
fn body(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("valid instruction fixture"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("valid instruction fixture")
}
#[test]
fn instructions_checked_save_and_publish_return_the_committed_version() {
  let root = project();
  let draft = editor::create_in(
    root.path(),
    Name::new("Standards").expect("valid instruction fixture"),
    &body("Original"),
  )
  .expect("valid instruction fixture");
  let published = editor::publish_in(root.path(), &draft).expect("valid instruction fixture");
  assert_eq!(published.state.published().expect("valid instruction fixture").get(), 1);
  assert!(!published.state.has_draft());
  let saved = editor::save_in(root.path(), &published, body("Draft")).expect("valid instruction fixture");
  assert_eq!(saved.based_on(), published.state.published());
  assert_eq!(saved.body.text().as_str(), "Draft");
  let next = editor::publish_in(root.path(), &saved).expect("valid instruction fixture");
  assert_eq!(next.state.published().expect("valid instruction fixture").get(), 2);
  assert_eq!(next.body.text().as_str(), "Draft");
}
#[test]
fn instructions_repeating_the_published_body_discards_the_draft() {
  let root = project();
  let original = editor::create_in(
    root.path(),
    Name::new("Standards").expect("valid instruction fixture"),
    &body("Original"),
  )
  .expect("valid instruction fixture");
  let original = editor::publish_in(root.path(), &original).expect("valid instruction fixture");
  let changed = editor::save_in(root.path(), &original, body("Changed")).expect("valid instruction fixture");
  let reverted = editor::save_in(root.path(), &changed, body("Original")).expect("valid instruction fixture");
  assert!(matches!(reverted.state, VersionState::Published(_)));
  assert!(
    ProjectDb::open(root.path())
      .expect("valid instruction fixture")
      .draft::<Instruction>(reverted.id)
      .expect("valid instruction fixture")
      .is_none()
  );
}
#[test]
fn instructions_delayed_save_and_publish_refuse_another_writers_draft() {
  let root = project();
  let original = editor::create_in(
    root.path(),
    Name::new("Standards").expect("valid instruction fixture"),
    &body("Original"),
  )
  .expect("valid instruction fixture");
  let outside = editor::save_in(root.path(), &original, body("Outside")).expect("valid instruction fixture");
  let save_error = editor::save_in(root.path(), &original, body("Delayed")).expect_err("an outside change is refused");
  assert_eq!(save_error.key, "errors.draftChanged");
  assert!(editor::needs_reload(&save_error));
  let publish_error = editor::publish_in(root.path(), &original).expect_err("an outside change is refused");
  assert_eq!(publish_error.key, "errors.draftChanged");
  assert!(editor::needs_reload(&publish_error));
  assert_eq!(
    ProjectInstructions::load_in(root.path())
      .expect("valid instruction fixture")
      .entry(original.id),
    Some(&outside)
  );
}
#[test]
fn instructions_stale_and_blank_drafts_remain_saved_after_publication_is_refused() {
  let root = project();
  let blank = editor::create_in(
    root.path(),
    Name::new("Blank").expect("valid instruction fixture"),
    &body(""),
  )
  .expect("valid instruction fixture");
  let blank_error = editor::publish_in(root.path(), &blank).expect_err("blank publication is refused");
  assert!(!editor::needs_reload(&blank_error));
  let initial = editor::create_in(
    root.path(),
    Name::new("Standards").expect("valid instruction fixture"),
    &body("Original"),
  )
  .expect("valid instruction fixture");
  editor::publish_in(root.path(), &initial).expect("valid instruction fixture");
  ProjectDb::open(root.path())
    .expect("valid instruction fixture")
    .save_draft(initial.id, &Draft::new(body("Stale"), None, Timestamp::now()))
    .expect("valid instruction fixture");
  let stale = ProjectInstructions::load_in(root.path())
    .expect("valid instruction fixture")
    .entry(initial.id)
    .expect("valid instruction fixture")
    .clone();
  assert!(editor::publish_in(root.path(), &stale).is_err());
  assert_eq!(
    ProjectInstructions::load_in(root.path())
      .expect("valid instruction fixture")
      .entry(initial.id),
    Some(&stale)
  );
}
