//! Saving a pipeline's edit with a new name (PV2-32): a rename alone saves the name of a local pipeline and publishes
//! no version; a team pipeline keeps the name the team server has (no rename is sent to it), so its rename is refused
//! with why, alone or with changed steps, and nothing is stored. `cargo test -p orchester-workspace --test
//! pipeline_save_rename rename`.
use std::collections::HashMap;

use orchester_core::{
  id::{ProjectId, ResourceId, ServerId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandProgram, Pipeline, PipelineEdit, Slug, StepKind, Tool, ToolAction},
  sharing::MemberRole,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, SharingLink};
use orchester_workspace::pipelines::{
  data::OpenedPipeline,
  write::{Allowed, WriteError, refusals, save_named_in},
};

fn project(shared: bool) -> (tempfile::TempDir, ProjectDb) {
  let folder = tempfile::tempdir().expect("a valid fixture");
  std::fs::create_dir(folder.path().join(".git")).expect("a valid fixture");
  let mut db = ProjectDb::create(
    folder.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("TEST").expect("a valid fixture"),
      Name::new("Test").expect("a valid fixture"),
    ),
  )
  .expect("a valid fixture");
  if shared {
    let link = SharingLink {
      server: ServerId::generate(),
      project: ProjectId::generate(),
      key: ProjectKey::new("TEST").expect("a valid fixture"),
      role: Some(MemberRole::Owner),
      joined_at: Timestamp::now(),
      cursor: 0,
      last_synced_at: Timestamp::now(),
      removed_at: None,
      address: None,
    };
    db.start_sharing(&link, &[], (&[], &HashMap::new()))
      .expect("a valid fixture");
  }
  (folder, db)
}

/// The pipeline *Checks*, one step running the published tool *Cargo*, created as its first version (D-032).
fn pipeline(db: &mut ProjectDb) -> ResourceId {
  let tool = Tool::new(
    "",
    ToolAction::Command(CommandAction::new(
      CommandProgram::new("cargo").expect("a valid fixture"),
      vec![],
    )),
    None,
  )
  .expect("a valid fixture");
  let (tool, _) = db
    .create_published(
      Slug::new("cargo").expect("a valid fixture"),
      Name::new("Cargo").expect("a valid fixture"),
      &tool,
      Timestamp::now(),
    )
    .expect("a valid fixture");
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("a valid fixture"));
  let key = edit.add_step(Name::new("Build").expect("a valid fixture"), None);
  edit.update(&key, |step| step.kind = StepKind::Tool { tool: Some(tool.id()) });
  db.create_published(
    Slug::new("checks").expect("a valid fixture"),
    Name::new("Checks").expect("a valid fixture"),
    &edit.validate().expect("a valid fixture"),
    Timestamp::now(),
  )
  .expect("a valid fixture")
  .0
  .id()
}

fn renamed() -> Name {
  Name::new("Gates").expect("a valid fixture")
}

#[test]
fn rename_alone_of_a_local_pipeline_saves_the_name_and_no_version() {
  let (folder, mut db) = project(false);
  let id = pipeline(&mut db);
  let before = OpenedPipeline::load(&db, id).expect("a valid fixture");
  assert!(!before.team);
  assert!(refusals(&before, &before.body, &renamed()).is_empty());

  let saved = save_named_in(folder.path(), &before, &before.body, &renamed(), &Allowed::default()).expect("saved");

  assert_eq!(&*saved.name, "Gates");
  assert_eq!(saved.version, before.version, "a rename alone publishes no version");
  assert_eq!(db.versions::<Pipeline>(id).expect("a valid fixture").len(), 1);
}

#[test]
fn rename_of_a_team_pipeline_is_refused_with_why_and_stores_nothing() {
  let (folder, mut db) = project(true);
  let id = pipeline(&mut db);
  let before = OpenedPipeline::load(&db, id).expect("a valid fixture");
  assert!(before.team, "the pipeline is the team's");
  let mut edit = PipelineEdit::new(before.body.clone());
  edit.set_description("Changed.");
  let changed = edit.validate().expect("a valid fixture");

  for body in [&before.body, &changed] {
    let why = refusals(&before, body, &renamed());
    assert_eq!(
      why.first().map(|message| message.key),
      Some("pipelines.problems.teamRename")
    );
    let refused = save_named_in(folder.path(), &before, body, &renamed(), &Allowed::default());
    assert!(
      matches!(&refused, Err(WriteError::NotSaved(reasons)) if reasons[0].key == "pipelines.problems.teamRename"),
      "{refused:?}"
    );
  }
  let after = OpenedPipeline::load(&db, id).expect("a valid fixture");
  assert_eq!(&*after.name, "Checks", "the name stays the team's");
  assert_eq!(after.version, before.version, "no version is published");
}

#[test]
fn rename_kept_as_the_team_has_it_lets_a_team_pipeline_save_its_steps() {
  let (folder, mut db) = project(true);
  let id = pipeline(&mut db);
  let before = OpenedPipeline::load(&db, id).expect("a valid fixture");
  let mut edit = PipelineEdit::new(before.body.clone());
  edit.set_description("Changed.");
  let changed = edit.validate().expect("a valid fixture");
  let name = Name::new("Checks").expect("a valid fixture");

  assert_eq!(refusals(&before, &changed, &name), Vec::new());
  let saved = save_named_in(folder.path(), &before, &changed, &name, &Allowed::default()).expect("saved");

  assert_eq!(saved.version, before.version.next().expect("a next version"));
  assert_eq!(saved.body.description(), "Changed.");
}
