//! Saving a pipeline's edit with a new name (PV2-32, PV2-34): a rename alone saves the name of a local pipeline and
//! saves no version. A team pipeline's save, a rename too, is the team server's to take (D-042, D-043,
//! `team_pipeline_save`). `cargo test -p orchester-workspace --test
//! pipeline_save_rename rename`.
#[path = "startup_join/support.rs"]
#[allow(
  dead_code,
  reason = "the joins' desktop, of which these tests use the session's servers"
)]
mod support;

use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandProgram, Pipeline, PipelineEdit, Slug, StepKind, Tool, ToolAction},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::pipelines::{
  data::OpenedPipeline,
  write::{Allowed, WriteError, refusals, save_named_in},
};

/// Saves `body`, an edit of `before`, named `name`, in the local project at `root`, which reaches no team server.
fn save_here_in(
  root: &std::path::Path,
  before: &OpenedPipeline,
  body: &Pipeline,
  name: &Name,
  allowed: &Allowed,
) -> Result<OpenedPipeline, WriteError> {
  let desk = support::Desk::new();
  save_named_in(root, &desk.servers, before, body, name, allowed).map(|stored| stored.opened)
}

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
  let (folder, mut db) = project();
  let id = pipeline(&mut db);
  let before = OpenedPipeline::load(&db, id).expect("a valid fixture");
  assert!(!before.team);
  assert!(refusals(&before, &before.body).is_empty());

  let saved = save_here_in(folder.path(), &before, &before.body, &renamed(), &Allowed::default()).expect("saved");

  assert_eq!(&*saved.name, "Gates");
  assert_eq!(saved.version, before.version, "a rename alone publishes no version");
  assert_eq!(db.versions::<Pipeline>(id).expect("a valid fixture").len(), 1);
}

#[test]
fn rename_or_save_of_a_pipeline_the_team_server_no_longer_has_is_said_in_words() {
  use orchester_workspace::sharing::{NotTaken, team_edit::Refusal};

  let gone = NotTaken::Refused(Refusal::of_code("not_found"));

  assert_eq!(gone.message().key, "pipelines.problems.teamGone");
  assert_eq!(gone.code(), "team_server_refused");
}
