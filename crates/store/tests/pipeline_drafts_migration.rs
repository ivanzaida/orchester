//! The migration by which pipelines have no drafts (D-032, D-033): a database before it opens with every pipeline draft
//! deleted, a local one's, one over a version and an admin's of a team pipeline, and every pipeline with no version
//! deleted with what names it; the other kinds keep their drafts, a version a run pinned or the team's stays, a shared
//! task naming a deleted pipeline reads it as missing, and the references the database checks still hold. A project
//! without pipelines is left as it is.
//! `cargo test -p orchester-store --test pipeline_drafts_migration pipeline_drafts_migration`.

mod support;

use std::{collections::BTreeSet, fs};

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId, TaskId},
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, Draft, Pipeline, PromptText, ProviderSettings, RepositoryAccess, ResourceKind, Slug, Step, StepKey,
    StepKind, VersionNumber,
  },
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
};
use orchester_store::{ProjectDb, ProjectMigration};
use rusqlite::{Connection, params};
use support::{at, copied_at_version, model, name, started_run};
use tempfile::TempDir;

fn project() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    name("Kontur"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("project");
  (repo, db)
}

fn database(repo: &TempDir) -> std::path::PathBuf {
  repo.path().join(".orchester").join("orchester.db")
}

fn profile(responsibility: &str) -> AgentProfile {
  AgentProfile::new(
    name("Builder"),
    responsibility.into(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    model(),
    ProviderSettings::default(),
  )
}

fn pipeline(keys: &[&str], profile: ResourceId) -> Pipeline {
  let steps = keys
    .iter()
    .map(|key| Step {
      key: StepKey::new(key).expect("key"),
      name: name(key),
      kind: StepKind::Agent {
        profile: Some(profile),
        repository: RepositoryAccess::Write,
        model: None,
        permissions: StepPermissions::default(),
      },
      instructions: PromptText::new("").expect("prompt"),
      depends_on: BTreeSet::new(),
      approval_gate: false,
      on_failure: None,
      budget: None,
    })
    .collect();
  Pipeline::new(steps).expect("a pipeline")
}

fn task(db: &mut ProjectDb, title: &str) -> TaskId {
  let details = TaskDetails {
    title: name(title),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  db.create_tasks(IdempotencyKey::generate(), vec![details], at(1))
    .expect("a task")[0]
    .id()
}

/// What the database held before the migration.
struct Seeded {
  builder: ResourceId,
  /// A pipeline that is only a draft: the default, imported (from a project and from a repository), archived, chosen
  /// by `sketch_task` and named by `team_task`, a shared task, by its resource id.
  sketch: ResourceId,
  sketch_task: TaskId,
  team_task: TaskId,
  /// v2 with a draft over it, saved by automation, and chosen by `delivery_task`.
  delivery: ResourceId,
  delivery_task: TaskId,
  /// The team's, its v1 kept, with an admin's local draft over it.
  team_flow: ResourceId,
}

/// Seeds through the store what it can still make, and the drafts and the draft-only pipeline in raw rows, as an older
/// build left them. The project has a run started on a pipeline of its own.
fn seeded(repo: &TempDir, db: &mut ProjectDb) -> Seeded {
  let (builder, _) = db
    .create_published(Slug::new("maker").expect("slug"), name("Builder"), &profile(""), at(1))
    .expect("profile");
  let builder = builder.id();
  db.save_draft(builder, &Draft::new(profile("Kept"), Some(VersionNumber::FIRST), at(2)))
    .expect("a profile's draft");
  let (delivery, _) = db
    .published_pipeline_for_test("shipping", "Shipping", &pipeline(&["implement"], builder), at(2))
    .expect("delivery");
  db.publish_pipeline_edit(
    delivery,
    &pipeline(&["plan"], builder),
    VersionNumber::FIRST,
    None,
    at(3),
  )
  .expect("v2");
  let (team_flow, _) = db
    .published_pipeline_for_test("team-flow", "Team flow", &pipeline(&["implement"], builder), at(5))
    .expect("team flow");
  let sketch_task = task(db, "Sketch it");
  let delivery_task = task(db, "Deliver it");
  let team_task = task(db, "Share it");
  let sketch = ResourceId::generate();
  let raw = Connection::open(database(repo)).expect("the database");
  raw_rows(
    &raw,
    [sketch, delivery, team_flow],
    [sketch_task, delivery_task, team_task],
  );
  Seeded {
    builder,
    sketch,
    sketch_task,
    team_task,
    delivery,
    delivery_task,
    team_flow,
  }
}

/// The draft-only Sketch, with what names it, the drafts over Delivery's v2 and Team flow's v1, and the marks the
/// migration reads or keeps on them.
fn raw_rows(
  raw: &Connection,
  [sketch, delivery, team_flow]: [ResourceId; 3],
  [sketch_task, delivery_task, team_task]: [TaskId; 3],
) {
  let body: String = raw
    .query_row(
      "SELECT body FROM resource_versions WHERE resource_id = ?1",
      [team_flow.to_string()],
      |row| row.get(0),
    )
    .expect("a pipeline body");
  let rows = statements(
    [sketch.to_string(), delivery.to_string(), team_flow.to_string()],
    [sketch_task, delivery_task, team_task],
    body,
  );
  for (sql, values) in rows {
    raw
      .execute(sql, rusqlite::params_from_iter(values))
      .unwrap_or_else(|error| panic!("{sql}: {error}"));
  }
  raw
    .execute(
      "UPDATE resources SET sharing = 'shared', server_revision = 1 WHERE id = ?1",
      params![team_flow.to_string()],
    )
    .expect("the team's");
}

/// The rows [`raw_rows`] writes, with their values: the pipelines' ids, the tasks' and Team flow's body.
fn statements(
  [sketch, delivery, team_flow]: [String; 3],
  [sketch_task, delivery_task, team_task]: [TaskId; 3],
  body: String,
) -> [(&'static str, Vec<String>); 14] {
  [
    (
      "INSERT INTO resources (id, kind, slug, name, created_at) VALUES (?1, 'pipeline', 'sketch', 'Sketch', 7)",
      vec![sketch.clone()],
    ),
    (
      "INSERT INTO resource_drafts (resource_id, body, based_on, updated_at) VALUES (?1, ?2, NULL, 7)",
      vec![sketch.clone(), body.clone()],
    ),
    (
      "INSERT INTO resource_drafts (resource_id, body, based_on, updated_at) VALUES (?1, ?2, 2, 4)",
      vec![delivery.clone(), body.clone()],
    ),
    (
      "INSERT INTO resource_drafts (resource_id, body, based_on, updated_at) VALUES (?1, ?2, 1, 6)",
      vec![team_flow, body],
    ),
    (
      "INSERT INTO project_default_pipeline (id, pipeline_id) VALUES (1, ?1)",
      vec![sketch.clone()],
    ),
    (
      "INSERT INTO resource_imports (resource_id, import_key, source_project_id, source_project_name, \
       source_resource_id, source_version, imported_at) VALUES (?1, 'import-1', 'p', 'Atlas', 's', NULL, 7)",
      vec![sketch.clone()],
    ),
    (
      "INSERT INTO repository_imports (resource_id, import_key, source, imported_at) VALUES (?1, 'files-1', 'AGENTS.md', 7)",
      vec![sketch.clone()],
    ),
    (
      "INSERT INTO removed_resources (resource_id, removed_at) VALUES (?1, 8)",
      vec![sketch.clone()],
    ),
    (
      "INSERT INTO automation_drafts (resource_id, saved_at) VALUES (?1, 7)",
      vec![sketch.clone()],
    ),
    (
      "INSERT INTO automation_drafts (resource_id, saved_at) VALUES (?1, 4)",
      vec![delivery.clone()],
    ),
    (
      "INSERT INTO money_step_budgets_before_tokens (owner, owner_id, step_key, micros, currency) \
       VALUES ('draft', ?1, 'implement', 5, 'USD')",
      vec![sketch.clone()],
    ),
    (
      "UPDATE tasks SET pipeline_id = ?1 WHERE id = ?2",
      vec![sketch.clone(), sketch_task.to_string()],
    ),
    (
      "UPDATE tasks SET sharing = 'shared', pipeline_slug = ?1 WHERE id = ?2",
      vec![sketch, team_task.to_string()],
    ),
    (
      "UPDATE tasks SET pipeline_id = ?1 WHERE id = ?2",
      vec![delivery, delivery_task.to_string()],
    ),
  ]
}

/// The project's database rewound to before the migration, opened again so it applies.
fn migrated(repo: &TempDir) -> (TempDir, ProjectDb) {
  let old = copied_at_version(
    &database(repo),
    ProjectDb::version_before(ProjectMigration::PipelineDrafts),
  );
  let db = ProjectDb::open(old.path()).expect("migrated, every reference the database checks still holding");
  (old, db)
}

fn count(raw: &Connection, sql: &str, id: ResourceId) -> i64 {
  raw
    .query_row(sql, [id.to_string()], |row| row.get(0))
    .unwrap_or_else(|error| panic!("{sql}: {error}"))
}

#[test]
fn pipeline_drafts_migration_deletes_every_pipeline_draft_and_the_pipelines_with_no_version() {
  let (repo, mut db, run, _) = started_run();
  let seeded = seeded(&repo, &mut db);
  drop(db);

  let (old, db) = migrated(&repo);

  let pinned = run.snapshot().pipeline();
  let ran = db
    .version::<Pipeline>(pinned.resource, pinned.number)
    .expect("read")
    .expect("the version a run pinned stays");
  assert_eq!(ran.id(), pinned.version);
  assert!(db.run(run.id()).expect("read").is_some(), "the run stays");
  assert_eq!(
    db.missing_pipelines().expect("read").get(&seeded.team_task),
    Some(&seeded.sketch.to_string()),
    "a shared task naming the deleted pipeline reads it as missing, so an edit sends no pipeline change"
  );

  assert_eq!(
    db.resource(seeded.sketch).expect("read"),
    None,
    "a draft-only pipeline is gone"
  );
  assert!(
    !db
      .slugs(ResourceKind::Pipeline)
      .expect("slugs")
      .iter()
      .any(|slug| slug.as_str() == "sketch"),
    "its slug is free again"
  );
  assert_eq!(db.default_pipeline().expect("default"), None);
  let sketch_task = db.task(seeded.sketch_task).expect("read").expect("the task stays");
  assert_eq!(sketch_task.details().pipeline, None, "the task that chose it has none");
  let delivery_task = db.task(seeded.delivery_task).expect("read").expect("the task");
  assert_eq!(delivery_task.details().pipeline, Some(seeded.delivery));

  for pipeline in [seeded.delivery, seeded.team_flow] {
    assert_eq!(
      db.draft::<Pipeline>(pipeline).expect("read"),
      None,
      "no pipeline keeps a draft"
    );
  }
  let delivery = db
    .latest_version::<Pipeline>(seeded.delivery)
    .expect("read")
    .expect("v2 stays");
  assert_eq!(delivery.number().get(), 2);
  assert_eq!(db.versions::<Pipeline>(seeded.team_flow).expect("read").len(), 1);
  let kept = db
    .draft::<AgentProfile>(seeded.builder)
    .expect("read")
    .expect("a profile keeps its draft");
  assert_eq!(kept.body().responsibilities(), "Kept");
  drop(db);
  nothing_names_the_deleted(&old, &seeded);
}

/// Nothing of the migrated database still names Sketch or the drafts: its imports, removal, automation marks and money
/// budgets, the drafts' marks; every reference holds.
fn nothing_names_the_deleted(old: &TempDir, seeded: &Seeded) {
  let raw = Connection::open(old.path().join(".orchester").join("orchester.db")).expect("the migrated database");
  for (sql, id) in [
    (
      "SELECT COUNT(*) FROM resource_imports WHERE resource_id = ?1",
      seeded.sketch,
    ),
    (
      "SELECT COUNT(*) FROM removed_resources WHERE resource_id = ?1",
      seeded.sketch,
    ),
    (
      "SELECT COUNT(*) FROM automation_drafts WHERE resource_id = ?1",
      seeded.sketch,
    ),
    (
      "SELECT COUNT(*) FROM automation_drafts WHERE resource_id = ?1",
      seeded.delivery,
    ),
    (
      "SELECT COUNT(*) FROM money_step_budgets_before_tokens WHERE owner_id = ?1",
      seeded.sketch,
    ),
    (
      "SELECT COUNT(*) FROM repository_imports WHERE resource_id = ?1",
      seeded.sketch,
    ),
  ] {
    assert_eq!(count(&raw, sql, id), 0, "{sql}");
  }
  let references: i64 = raw
    .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0))
    .expect("check");
  assert_eq!(references, 0);
}

#[test]
fn pipeline_drafts_migration_changes_nothing_in_a_project_without_pipelines() {
  let (repo, mut db) = project();
  let (builder, _) = db
    .create_published(Slug::new("maker").expect("slug"), name("Builder"), &profile(""), at(1))
    .expect("profile");
  db.save_draft(
    builder.id(),
    &Draft::new(profile("Kept"), Some(VersionNumber::FIRST), at(2)),
  )
  .expect("a profile's draft");
  let task = task(&mut db, "Plain");
  drop(db);

  let (_old, db) = migrated(&repo);

  assert!(db.resources(ResourceKind::Pipeline).expect("read").is_empty());
  assert_eq!(db.resources(ResourceKind::AgentProfile).expect("read").len(), 1);
  let kept = db
    .draft::<AgentProfile>(builder.id())
    .expect("read")
    .expect("the profile keeps its draft");
  assert_eq!(kept.body().responsibilities(), "Kept");
  assert!(db.task(task).expect("read").is_some(), "the task stays");
}
