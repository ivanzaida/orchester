//! Renaming a pipeline changes its name alone, and the project's default is read back. `cargo test -p
//! orchester-workspace --test pipeline_rename`.
use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{Pipeline, Slug},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::pipelines::{
  default::default_in,
  rename::{RenameError, rename_in},
};

fn project() -> (tempfile::TempDir, ProjectDb) {
  let folder = tempfile::tempdir().expect("a valid fixture");
  std::fs::create_dir(folder.path().join(".git")).expect("a valid fixture");
  let db = ProjectDb::create(
    folder.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("TEST").expect("a valid fixture"),
      Name::new("Test").expect("a valid fixture"),
    ),
  )
  .expect("a valid fixture");
  (folder, db)
}

fn pipeline(db: &mut ProjectDb, name: &str) -> ResourceId {
  let body = Pipeline::new(vec![]).expect("a valid fixture");
  db.create_resource(
    Slug::from_name(name).expect("a valid fixture"),
    Name::new(name).expect("a valid fixture"),
    &body,
    Timestamp::now(),
  )
  .expect("a valid fixture")
  .id()
}

fn name_of(db: &ProjectDb, id: ResourceId) -> String {
  db.resource(id)
    .expect("a valid fixture")
    .expect("the pipeline")
    .name()
    .as_str()
    .to_owned()
}

#[test]
fn rename_changes_the_name_and_keeps_the_slug_and_the_draft() {
  let (folder, mut db) = project();
  let id = pipeline(&mut db, "Feature delivery");
  let saved = rename_in(folder.path(), id, "  Feature flow  ").expect("a valid fixture");
  assert_eq!(
    saved.as_str(),
    "Feature flow",
    "it answers the name as it was saved, trimmed"
  );
  let db = ProjectDb::open(folder.path()).expect("a valid fixture");
  let resource = db.resource(id).expect("a valid fixture").expect("the pipeline");
  assert_eq!(resource.name().as_str(), "Feature flow");
  assert_eq!(resource.slug().as_str(), "feature-delivery");
  assert!(db.draft::<Pipeline>(id).expect("a valid fixture").is_some());
}

#[test]
fn rename_may_change_only_the_case_of_its_own_name() {
  let (folder, mut db) = project();
  let id = pipeline(&mut db, "Feature delivery");
  rename_in(folder.path(), id, "FEATURE DELIVERY").expect("a valid fixture");
  assert_eq!(
    name_of(&ProjectDb::open(folder.path()).expect("a valid fixture"), id),
    "FEATURE DELIVERY"
  );
}

#[test]
fn rename_refuses_another_pipelines_name_even_an_archived_ones() {
  let (folder, mut db) = project();
  let id = pipeline(&mut db, "Feature delivery");
  let other = pipeline(&mut db, "Quick fix");
  let archived = pipeline(&mut db, "Old flow");
  db.archive_local_resource(archived, Timestamp::now())
    .expect("a valid fixture");
  for taken in ["quick fix", "Quick-Fix", "old flow"] {
    let error = rename_in(folder.path(), id, taken).expect_err(taken);
    assert!(matches!(error, RenameError::Taken { .. }), "{taken}: {error:?}");
    assert_eq!(error.message().key, "pipelines.rename.taken");
  }
  let db = ProjectDb::open(folder.path()).expect("a valid fixture");
  assert_eq!(name_of(&db, id), "Feature delivery");
  assert_eq!(name_of(&db, other), "Quick fix");
}

#[test]
fn rename_refuses_a_blank_or_too_long_name() {
  let (folder, mut db) = project();
  let id = pipeline(&mut db, "Feature delivery");
  let blank = rename_in(folder.path(), id, "   ").expect_err("refused");
  assert_eq!(blank.message().key, "pipelines.rename.blank");
  let long = rename_in(folder.path(), id, &"a".repeat(Name::MAX_CHARS + 1)).expect_err("refused");
  assert_eq!(long.message().key, "pipelines.rename.tooLong");
}

#[test]
fn default_in_reads_what_the_project_stored() {
  let (folder, mut db) = project();
  let id = pipeline(&mut db, "Feature delivery");
  assert_eq!(default_in(folder.path()).expect("a valid fixture"), None);
  db.save_default_pipeline(Some(id)).expect("a valid fixture");
  assert_eq!(default_in(folder.path()).expect("a valid fixture"), Some(id));
}
