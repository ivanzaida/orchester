//! A stale editor must never overwrite another saved draft.
use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::{Draft, Pipeline, PipelineEdit, Slug},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

#[test]
fn pipeline_draft_matching_rejects_changed_body_without_overwriting() {
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
  let original = Pipeline::new(vec![]).unwrap();
  let id = db
    .create_resource(
      Slug::new("test").unwrap(),
      Name::new("Test").unwrap(),
      &original,
      Timestamp::now(),
    )
    .unwrap()
    .id();
  let mut edit = PipelineEdit::new(original.clone());
  edit.set_description("Other editor");
  let external = edit.validate().unwrap();
  db.save_draft(id, &Draft::new(external.clone(), None, Timestamp::now()))
    .unwrap();
  let stale = db.save_draft_matching(
    id,
    &original,
    None,
    &Draft::new(original.clone(), None, Timestamp::now()),
  );
  assert!(matches!(stale, Err(StoreError::DraftChanged(_))));
  assert_eq!(db.draft::<Pipeline>(id).unwrap().unwrap().body(), &external);
  edit.set_description("Current editor");
  let changed = edit.validate().unwrap();
  db.save_draft_matching(
    id,
    &external,
    None,
    &Draft::new(changed.clone(), None, Timestamp::now()),
  )
  .unwrap();
  assert_eq!(db.draft::<Pipeline>(id).unwrap().unwrap().body(), &changed);
}
