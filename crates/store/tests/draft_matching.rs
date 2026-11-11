//! A stale editor must never overwrite another saved draft. The mechanics are shared by every kind that holds drafts;
//! an instruction stands for them (a pipeline holds none, D-032).
#[path = "draft_matching/concurrency.rs"]
mod concurrency;

use std::collections::BTreeSet;

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::{AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, PromptText, Slug},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

fn instruction(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).unwrap(),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .unwrap()
}

#[test]
fn draft_matching_rejects_changed_body_without_overwriting() {
  let folder = tempfile::tempdir().unwrap();
  std::fs::create_dir(folder.path().join(".git")).unwrap();
  let mut db = ProjectDb::create(
    folder.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("TEST").unwrap(),
      Name::new("Test").unwrap(),
    ),
  )
  .unwrap();
  let original = instruction("Original.");
  let id = db
    .create_resource(
      Slug::new("test").unwrap(),
      Name::new("Test").unwrap(),
      &original,
      Timestamp::now(),
    )
    .unwrap()
    .id();
  let external = instruction("Other editor.");
  db.save_draft(id, &Draft::new(external.clone(), None, Timestamp::now()))
    .unwrap();
  let stale = db.save_draft_matching(
    id,
    &original,
    None,
    &Draft::new(original.clone(), None, Timestamp::now()),
  );
  assert!(matches!(stale, Err(StoreError::DraftChanged(_))));
  assert_eq!(db.draft::<Instruction>(id).unwrap().unwrap().body(), &external);
  let changed = instruction("Current editor.");
  db.save_draft_matching(
    id,
    &external,
    None,
    &Draft::new(changed.clone(), None, Timestamp::now()),
  )
  .unwrap();
  assert_eq!(db.draft::<Instruction>(id).unwrap().unwrap().body(), &changed);
}
