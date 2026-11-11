//! The editor and automation share the permission checks in workspace, and a pipeline is stored only as published
//! versions (D-032): a save publishes the next version or is refused with every reason, and nothing is stored then.
//! `cargo test -p orchester-workspace --test pipeline_writes pipeline_writes`.
use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandProgram, Pipeline, PipelineEdit, Slug, StepKind, Tool, ToolAction},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use orchester_workspace::pipelines::{
  data::OpenedPipeline,
  library::{CreateError, create_with},
  widening::save_reasons,
  write::{self, Allowed, WriteError},
};
use tempfile::TempDir;

fn project() -> (TempDir, ProjectDb) {
  let folder = tempfile::tempdir().expect("a temporary folder");
  std::fs::create_dir(folder.path().join(".git")).expect("a .git folder");
  let db = ProjectDb::create(
    folder.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("TEST").expect("key"),
      Name::new("Test").expect("name"),
    ),
  )
  .expect("the project database");
  (folder, db)
}

/// A published command tool a step can run.
fn tool(db: &mut ProjectDb) -> ResourceId {
  let tool = Tool::new(
    "",
    ToolAction::Command(CommandAction::new(
      CommandProgram::new("cargo").expect("program"),
      vec![],
    )),
    None,
  )
  .expect("a command tool");
  db.create_published(
    Slug::new("tool").expect("slug"),
    Name::new("Tool").expect("name"),
    &tool,
    Timestamp::now(),
  )
  .expect("a published tool")
  .0
  .id()
}

/// One step that runs `tool`, waiting for approval when `gate`.
fn checks(tool: ResourceId, gate: bool) -> Pipeline {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("an empty pipeline"));
  let key = edit.add_step(Name::new("Check").expect("name"), None);
  edit.update(&key, |step| {
    step.kind = StepKind::Tool { tool: Some(tool) };
    step.approval_gate = gate;
  });
  edit.validate().expect("a valid pipeline")
}

#[test]
fn pipeline_writes_publish_the_next_version_and_need_a_person_to_drop_a_gate() {
  let (folder, mut db) = project();
  let tool = tool(&mut db);
  let body = checks(tool, true);
  let id = create_with(&mut db, Name::new("Pipeline").unwrap(), &body, Timestamp::now()).unwrap();
  let first = OpenedPipeline::load(&db, id).unwrap();
  assert_eq!(first.version.get(), 1, "created as v1");
  let weaker = checks(tool, false);
  assert!(matches!(
    write::save_in(folder.path(), &first, &weaker, &Allowed::default()),
    Err(WriteError::PersonOnly)
  ));
  assert_eq!(db.versions::<Pipeline>(id).unwrap().len(), 1, "nothing stored");
  let reasons = save_reasons(Some(&first.body), &weaker, &first.assignables);
  let saved = write::save_in(folder.path(), &first, &weaker, &Allowed(reasons)).unwrap();
  assert_eq!(saved.version.get(), 2);
  assert_eq!(saved.body, weaker);
  assert_eq!(db.versions::<Pipeline>(id).unwrap()[0].body(), &body);
  assert!(db.draft::<Pipeline>(id).unwrap().is_none(), "a pipeline holds no draft");
}

#[test]
fn pipeline_writes_refuse_an_unpublishable_pipeline_with_every_reason_and_store_nothing() {
  use orchester_core::resource::{PromptText, StarterAssignments, StarterStepText, feature_delivery};
  let (folder, mut db) = project();
  let starter = feature_delivery(
    |step| StarterStepText {
      name: Name::new(step.key()).unwrap(),
      instructions: PromptText::new("").unwrap(),
    },
    StarterAssignments::default(),
  );
  let Err(CreateError::NotSaved(reasons)) =
    create_with(&mut db, Name::new("Starter").unwrap(), &starter, Timestamp::now())
  else {
    panic!("a starter without profiles or tools is refused");
  };
  assert_eq!(
    reasons.len(),
    starter.steps().len(),
    "every step without a profile or tool is named: {reasons:?}"
  );
  assert!(
    db.resources(orchester_core::resource::ResourceKind::Pipeline)
      .unwrap()
      .is_empty()
  );
  let empty = create_with(
    &mut db,
    Name::new("Empty").unwrap(),
    &Pipeline::new(vec![]).unwrap(),
    Timestamp::now(),
  );
  assert!(
    matches!(&empty, Err(CreateError::NotSaved(reasons)) if reasons[0].key == "pipelines.problems.noSteps"),
    "{empty:?}"
  );

  let tool = tool(&mut db);
  let id = create_with(
    &mut db,
    Name::new("Checks").unwrap(),
    &checks(tool, false),
    Timestamp::now(),
  )
  .unwrap();
  let opened = OpenedPipeline::load(&db, id).unwrap();
  let refused = write::save_in(folder.path(), &opened, &starter, &Allowed::default());
  assert!(
    matches!(&refused, Err(WriteError::NotSaved(reasons)) if reasons.len() == starter.steps().len()),
    "{refused:?}"
  );
  assert_eq!(db.versions::<Pipeline>(id).unwrap().len(), 1, "nothing stored");
  assert!(db.draft::<Pipeline>(id).unwrap().is_none());
}

#[test]
fn pipeline_writes_refuse_an_unchanged_save_and_one_made_from_an_older_version() {
  let (folder, mut db) = project();
  let tool = tool(&mut db);
  let body = checks(tool, false);
  let id = create_with(&mut db, Name::new("Pipeline").unwrap(), &body, Timestamp::now()).unwrap();
  let opened = OpenedPipeline::load(&db, id).unwrap();
  let unchanged = write::save_in(folder.path(), &opened, &body, &Allowed::default());
  assert!(
    matches!(&unchanged, Err(WriteError::NotSaved(reasons)) if reasons[0].key == "pipelines.problems.unchanged"),
    "{unchanged:?}"
  );
  write::save_in(
    folder.path(),
    &opened,
    &body.clone().with_description("First editor".into()),
    &Allowed::default(),
  )
  .unwrap();
  let stale = write::save_in(
    folder.path(),
    &opened,
    &body.clone().with_description("Second editor".into()),
    &Allowed::default(),
  );
  assert!(
    matches!(&stale, Err(error) if error.message().key == "pipelines.problems.staleDraft"),
    "{stale:?}"
  );
  let versions = db.versions::<Pipeline>(id).unwrap();
  assert_eq!(versions.len(), 2);
  assert_eq!(versions[1].body().description(), "First editor");
}

#[test]
fn pipeline_writes_rename_preserves_slug_and_rejects_stale_names_atomically() {
  let (folder, mut db) = project();
  let tool = tool(&mut db);
  let body = checks(tool, false);
  let id = create_with(&mut db, Name::new("Pipeline").unwrap(), &body, Timestamp::now()).unwrap();
  let before = OpenedPipeline::load(&db, id).unwrap();
  let after = write::save_named_in(
    folder.path(),
    &before,
    &body,
    &Name::new("Delivery").unwrap(),
    &Allowed::default(),
  )
  .unwrap();
  assert_eq!(&*after.name, "Delivery");
  assert_eq!(&*after.slug, "pipeline");
  assert_eq!(after.version.get(), 1, "a rename alone publishes no version");
  assert!(matches!(
    write::save_named_in(
      folder.path(),
      &before,
      &body.clone().with_description("stale edit".into()),
      &Name::new("Old editor").unwrap(),
      &Allowed::default()
    ),
    Err(WriteError::Store(StoreError::DraftChanged(_)))
  ));
  let current = OpenedPipeline::load(&db, id).unwrap();
  assert_eq!(&*current.name, "Delivery");
  assert!(current.body.description().is_empty());
  assert_eq!(current.version.get(), 1);
}
