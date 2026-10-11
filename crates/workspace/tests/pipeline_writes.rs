//! The editor and automation share the permission checks in workspace.
use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandProgram, Pipeline, PipelineEdit, Slug, StepKind, Tool, ToolAction},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::pipelines::{
  data::OpenedPipeline,
  widening::{draft_reasons, publish_reasons},
  write::{self, Allowed, WriteError},
};

#[test]
fn pipeline_writes_require_person_for_first_publish_and_removing_gates() {
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
  let now = Timestamp::now();
  let tool = Tool::new(
    "",
    ToolAction::Command(CommandAction::new(CommandProgram::new("cargo").unwrap(), vec![])),
    None,
  )
  .unwrap();
  let tool_id = db
    .create_resource(Slug::new("tool").unwrap(), Name::new("Tool").unwrap(), &tool, now)
    .unwrap()
    .id();
  db.publish_draft::<Tool>(tool_id, now).unwrap();
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).unwrap());
  let key = edit.add_step(Name::new("Check").unwrap(), None);
  edit.update(&key, |step| {
    step.kind = StepKind::Tool { tool: Some(tool_id) };
    step.approval_gate = true;
  });
  let body = edit.validate().unwrap();
  let id = db
    .create_resource(
      Slug::new("pipeline").unwrap(),
      Name::new("Pipeline").unwrap(),
      &body,
      now,
    )
    .unwrap()
    .id();
  let before = OpenedPipeline::load(&db, id).unwrap();
  assert!(matches!(
    write::publish_in(folder.path(), &before, &Allowed::default()),
    Err(WriteError::PersonOnly)
  ));
  let first = write::publish_in(folder.path(), &before, &Allowed(publish_reasons(None, &body))).unwrap();
  edit.update(&key, |step| step.approval_gate = false);
  let weaker = edit.validate().unwrap();
  assert!(matches!(
    write::save_in(folder.path(), &first, weaker.clone(), &Allowed::default()),
    Err(WriteError::PersonOnly)
  ));
  let saved = write::save_in(
    folder.path(),
    &first,
    weaker.clone(),
    &Allowed(draft_reasons(&first, &weaker)),
  )
  .unwrap();
  assert!(matches!(
    write::publish_in(folder.path(), &saved, &Allowed::default()),
    Err(WriteError::PersonOnly)
  ));
  let reasons = publish_reasons(saved.latest.as_ref().map(|v| &v.body), &weaker);
  let published = write::publish_in(folder.path(), &saved, &Allowed(reasons)).unwrap();
  assert_eq!(published.latest.unwrap().number.get(), 2);
  assert_eq!(db.versions::<Pipeline>(id).unwrap()[0].body(), &body);
  assert!(OpenedPipeline::load(&db, id).unwrap().draft_saved_at.is_none());
}

#[test]
fn pipeline_writes_keep_unassigned_starter_drafts_but_publication_stays_strict() {
  use orchester_core::resource::{PromptText, StarterAssignments, StarterStepText, feature_delivery};
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
  let body = feature_delivery(
    |step| StarterStepText {
      name: Name::new(step.key()).unwrap(),
      instructions: PromptText::new("").unwrap(),
    },
    StarterAssignments::default(),
  );
  let id = db
    .create_resource(
      Slug::new("starter").unwrap(),
      Name::new("Starter").unwrap(),
      &body,
      Timestamp::now(),
    )
    .unwrap()
    .id();
  let opened = OpenedPipeline::load(&db, id).unwrap();
  let edited = body.with_description("Still choosing profiles".into());
  let saved = write::save_in(folder.path(), &opened, edited.clone(), &Allowed::default()).unwrap();
  assert_eq!(saved.body, edited);
  assert!(matches!(
    write::publish_in(folder.path(), &saved, &Allowed(publish_reasons(None, &edited))),
    Err(WriteError::Store(_))
  ));
  assert!(db.latest_version::<Pipeline>(id).unwrap().is_none());
  assert_eq!(db.draft::<Pipeline>(id).unwrap().unwrap().body(), &edited);
}
#[test]
fn pipeline_writes_rename_preserves_slug_and_rejects_stale_names_atomically() {
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
  let body = Pipeline::new(vec![]).unwrap();
  let id = db
    .create_resource(
      Slug::new("pipeline").unwrap(),
      Name::new("Pipeline").unwrap(),
      &body,
      Timestamp::now(),
    )
    .unwrap()
    .id();
  let before = OpenedPipeline::load(&db, id).unwrap();
  let after = write::save_named_in(
    folder.path(),
    &before,
    body.clone(),
    &Name::new("Delivery").unwrap(),
    &Allowed::default(),
  )
  .unwrap();
  assert_eq!(&*after.name, "Delivery");
  assert_eq!(&*after.slug, "pipeline");
  assert!(matches!(
    write::save_named_in(
      folder.path(),
      &before,
      body.with_description("stale edit".into()),
      &Name::new("Old editor").unwrap(),
      &Allowed::default()
    ),
    Err(WriteError::Store(orchester_store::StoreError::DraftChanged(_)))
  ));
  let current = OpenedPipeline::load(&db, id).unwrap();
  assert_eq!(&*current.name, "Delivery");
  assert!(current.body.description().is_empty());
}
