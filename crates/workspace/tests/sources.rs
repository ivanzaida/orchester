//! Sources as screens and automation change them: what a typed path may become, the project's and a task's lists
//! with the person's confirmation, what a task inherits, and a pipeline draft's lists.

use std::{fs, path::Path};

use orchester_core::{
  id::{IdempotencyKey, ProjectId, TaskId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    CommandAction, CommandProgram, Pipeline, PipelineEdit, PipelineSources, Slug, StepKind, Tool, ToolAction,
  },
  source::SourcePath,
  store::{ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  pipelines::sources::{self as drafted, Listing},
  sources::{
    check::{Kind, Place, Refusal},
    listed::State,
    project, task,
  },
};
use tempfile::TempDir;

fn project_db() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary project");
  fs::create_dir(repo.path().join(".git")).expect("git folder");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    Name::new("Kontur").expect("name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("project database");
  (repo, db)
}

fn source(path: &Path) -> SourcePath {
  SourcePath::new(&path.to_string_lossy()).expect("an absolute path")
}

/// A folder outside the project with `files` files in it.
fn docs(files: usize) -> TempDir {
  let folder = tempfile::tempdir().expect("temporary folder");
  for file in 0..files {
    fs::write(folder.path().join(format!("{file}.md")), "docs").expect("a file");
  }
  folder
}

fn new_task(db: &mut ProjectDb) -> TaskId {
  let details = TaskDetails {
    title: Name::new("Webhooks").expect("title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  db.create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
    .expect("a task")[0]
    .id()
}

#[test]
fn sources_check_reads_what_a_path_names() {
  let (repo, _db) = project_db();
  let place = Place::new(repo.path(), "Kontur", Vec::new());
  let folder = docs(3);
  let found = place
    .check(&folder.path().to_string_lossy())
    .expect("a readable folder");
  assert_eq!(
    (found.kind, found.files, found.holds_refused),
    (Kind::Folder, Some(3), false)
  );
  assert_eq!(found.message().key, "sources.check.folder");
  let file = folder.path().join("0.md");
  let found = place.check(&file.to_string_lossy()).expect("a readable file");
  assert_eq!(found.kind, Kind::File);
  assert_eq!(found.message().key, "sources.check.file");
  fs::create_dir(folder.path().join(".git")).expect("a git folder");
  let found = place.check(&folder.path().to_string_lossy()).expect("still a source");
  assert!(found.holds_refused);
  assert_eq!(found.message().key, "sources.check.folderWithRefused");
}

#[test]
fn sources_check_refuses_what_runs_cannot_or_need_not_read() {
  let (repo, _db) = project_db();
  let folder = docs(1);
  let place = Place::new(repo.path(), "Kontur", vec![source(folder.path())]);
  let refused = |typed: &Path| place.check(&typed.to_string_lossy()).expect_err("refused");
  assert!(matches!(place.check("  "), Err(Refusal::Path(_))));
  assert!(matches!(place.check("docs/webhooks"), Err(Refusal::Path(_))));
  assert_eq!(refused(&folder.path().join("missing")), Refusal::NotFound);
  assert_eq!(
    refused(&folder.path().join("0.md")),
    Refusal::Covered(source(folder.path()))
  );
  fs::create_dir(repo.path().join("docs")).expect("a project folder");
  let inside = refused(&repo.path().join("docs"));
  assert_eq!(
    inside,
    Refusal::InsideProject {
      project: "Kontur".into()
    }
  );
  assert!(!inside.is_error());
  let other = tempfile::tempdir().expect("another project");
  fs::create_dir_all(other.path().join("atlas").join(".git")).expect("its git folder");
  fs::create_dir_all(other.path().join("atlas").join(".orchester")).expect("its state");
  assert_eq!(refused(&other.path().join("atlas").join(".git")), Refusal::Git);
  assert_eq!(
    refused(&other.path().join("atlas").join(".orchester")),
    Refusal::OtherProject { name: "atlas".into() }
  );
  let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).expect("a home folder");
  let data = Path::new(&home).join(if cfg!(windows) { "AppData" } else { "Library" });
  assert_eq!(refused(&data), Refusal::Credentials);
  assert!(Refusal::Credentials.is_error());
}

#[test]
fn sources_of_the_project_are_added_once_confirmed_and_removed() {
  let (repo, _db) = project_db();
  let folder = docs(2);
  let path = source(folder.path());
  assert_eq!(
    project::add_in(repo.path(), &path, false)
      .expect_err("needs a person")
      .key,
    "sources.personOnly"
  );
  assert_eq!(project::widenings(&path)[0].key, "sources.widens.project");
  project::add_in(repo.path(), &path, true).expect("added");
  let listed = project::load_in(repo.path()).expect("read");
  assert_eq!(listed.len(), 1);
  assert_eq!(
    (listed[0].kind, listed[0].state),
    (Some(Kind::Folder), State::Available)
  );
  assert_eq!(
    project::add_in(repo.path(), &path, true).expect_err("listed").key,
    "sources.refused.covered"
  );
  let removed = project::remove_in(repo.path(), &path).expect("removed");
  assert!(removed.runs.is_empty());
  assert_eq!(removed.message().key, "sources.removed.noRun");
  assert!(project::load_in(repo.path()).expect("read").is_empty());
}

#[test]
fn sources_listed_but_gone_say_runs_start_without_them() {
  let (repo, mut db) = project_db();
  let folder = docs(0);
  let gone = folder.path().join("runbook.md");
  db.set_project_sources(&[source(&gone)]).expect("listed");
  let listed = project::load_in(repo.path()).expect("read");
  assert_eq!((listed[0].kind, listed[0].state), (None, State::Missing));
  assert_eq!(listed[0].problem().expect("a problem").key, "sources.listed.missing");
}

#[test]
fn sources_of_a_task_come_on_top_of_the_project_and_its_pipeline() {
  let (repo, mut db) = project_db();
  let (shared, spec, style) = (docs(1), docs(1), docs(1));
  db.set_project_sources(&[source(shared.path())]).expect("project's");
  let task = new_task(&mut db);
  let (pipeline, version) = published_pipeline(&mut db, &source(style.path()));
  let inherited = task::inherited_in(repo.path(), Some((pipeline, version))).expect("read");
  assert_eq!(inherited.project.len(), 1);
  assert_eq!(inherited.pipeline.len(), 1);
  assert_eq!(inherited.pipeline[0].origin().key, "sources.origin.step");
  let own = source(spec.path());
  assert!(task::add_in(repo.path(), task, Some((pipeline, version)), &own, false).is_err());
  task::add_in(repo.path(), task, Some((pipeline, version)), &own, true).expect("added");
  assert_eq!(
    task::own_in(repo.path(), task).expect("read"),
    std::slice::from_ref(&own)
  );
  assert_eq!(task::run_count(&inherited, &[own.clone(), source(shared.path())]), 3);
  let refused = task::add_in(
    repo.path(),
    task,
    Some((pipeline, version)),
    &source(style.path()),
    true,
  );
  assert_eq!(refused.expect_err("the step's").key, "sources.refused.covered");
  task::remove_in(repo.path(), task, &own).expect("removed");
  assert!(task::own_in(repo.path(), task).expect("read").is_empty());
  task::save_new_in(repo.path(), task, std::slice::from_ref(&own), true).expect("saved");
  assert_eq!(task::own_in(repo.path(), task).expect("read"), [own]);
}

/// A published pipeline whose one step lists `step_source`.
fn published_pipeline(
  db: &mut ProjectDb,
  step_source: &SourcePath,
) -> (orchester_core::id::ResourceId, orchester_core::resource::VersionNumber) {
  let now = Timestamp::now();
  let tool = Tool::new(
    "",
    ToolAction::Command(CommandAction::new(
      CommandProgram::new("cargo").expect("a program"),
      vec![],
    )),
    None,
  )
  .expect("a tool");
  let tool = db
    .create_resource(
      Slug::new("tool").expect("slug"),
      Name::new("Tool").expect("name"),
      &tool,
      now,
    )
    .expect("tool")
    .id();
  db.publish_draft::<Tool>(tool, now).expect("published tool");
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty"));
  let key = edit.add_step(Name::new("Review").expect("name"), None);
  edit.update(&key, |step| step.kind = StepKind::Tool { tool: Some(tool) });
  drafted::add(&mut edit, &Listing::Step(key), step_source.clone());
  let body = edit.validate().expect("valid");
  let pipeline = db
    .create_resource(
      Slug::new("delivery").expect("slug"),
      Name::new("Feature delivery").expect("name"),
      &body,
      now,
    )
    .expect("pipeline")
    .id();
  let version = db.publish_draft::<Pipeline>(pipeline, now).expect("published").number();
  (pipeline, version)
}

#[test]
fn sources_of_a_pipeline_draft_are_its_own_and_its_steps() {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty"));
  let implement = edit.add_step(Name::new("Implement").expect("name"), None);
  let (tokens, guide) = (docs(0), docs(0));
  drafted::add(&mut edit, &Listing::Pipeline, source(tokens.path()));
  drafted::add(&mut edit, &Listing::Step(implement.clone()), source(guide.path()));
  assert_eq!(edit.sources().pipeline(), [source(tokens.path())]);
  assert_eq!(edit.sources().step(&implement), [source(guide.path())]);
  let read_back = drafted::steps_read_back(&edit).expect("a step lists one");
  assert_eq!(read_back.key, "sources.steps.oneOnStep");
  drafted::remove(&mut edit, &Listing::Step(implement.clone()), &source(guide.path()));
  assert!(drafted::steps_read_back(&edit).is_none());
  edit.remove_step(&implement);
  let body = edit.validate().expect("valid");
  assert_eq!(
    body.sources(),
    &PipelineSources::new(vec![source(tokens.path())], Default::default())
  );
}
