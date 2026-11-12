//! The migration that drops a local project's drafts (decided by the person 2026-11-09: the Agents hub edits in place,
//! with no drafts): a local project's database before it, with drafts of published profiles, instructions, skills,
//! tools and pipelines and of an instruction never published, opens with the drafts of the published profile,
//! instruction, skill and tool gone, the pipeline's draft and the never-published instruction's draft kept, and the
//! published versions as they were; a team project's drafts stay; applying it again changes nothing.
//! `cargo test -p orchester-store --test local_drafts_migration local_drafts_migration`.

mod support;

use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::{
    AgentProfile, AgentTargets, CommandAction, CommandProgram, InjectionEvent, Instruction, InstructionScope, Pipeline,
    PromptText, ResourceBody, ResourceKind, Skill, Slug, Tool,
  },
  store::ResourceStore,
};
use orchester_store::{ProjectDb, ProjectMigration};
use rusqlite::Connection;
use support::{at, copied_at_version, name, pending_run};
use tempfile::TempDir;

fn rule(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("prompt"),
    AgentTargets::All,
    [InjectionEvent::Start].into(),
    InstructionScope::Project,
  )
  .expect("instruction")
}

fn skill() -> Skill {
  Skill::new(
    "Runs the checks.".into(),
    "Run them all.".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("skill")
}

fn tool() -> Tool {
  Tool::command(
    "Runs the tests.",
    CommandAction::new(CommandProgram::new("cargo").expect("program"), Vec::new()),
  )
}

/// `body` made as `slug` and published as v1.
fn published<B: ResourceBody>(db: &mut ProjectDb, slug: &str, body: &B) -> ResourceId {
  let id = db
    .create_resource(Slug::new(slug).expect("slug"), name(slug), body, at(10))
    .expect("create")
    .id();
  db.publish_draft::<B>(id, at(11)).expect("v1");
  id
}

/// The resources of a project with a run.
struct Seeded {
  profile: ResourceId,
  pipeline: ResourceId,
  instruction: ResourceId,
  skill: ResourceId,
  tool: ResourceId,
  /// An instruction never published: its draft is all of it.
  unpublished: ResourceId,
}

/// A project whose run pins the builder profile and the delivery pipeline, a published instruction, skill and tool, and
/// an instruction that has only a draft.
fn seeded() -> (TempDir, Seeded) {
  let (repo, mut db, run) = pending_run();
  let seeded = Seeded {
    profile: db.resources(ResourceKind::AgentProfile).expect("read")[0].id(),
    pipeline: run.snapshot().pipeline().resource,
    instruction: published(&mut db, "style", &rule("Use tabs.")),
    skill: published(&mut db, "checks", &skill()),
    tool: published(&mut db, "tests", &tool()),
    unpublished: db
      .create_resource(
        Slug::new("later").expect("slug"),
        name("later"),
        &rule("Later."),
        at(13),
      )
      .expect("a draft only")
      .id(),
  };
  (repo, seeded)
}

/// The database of `repo` rewound to before the migration, with a draft of every published resource on its latest
/// version as the old pages' autosave left them, `prepare`d, and opened again so the migration applies.
fn migrated(repo: &TempDir, prepare: &str) -> (TempDir, ProjectDb) {
  let old = copied_at_version(
    &repo.path().join(".orchester").join("orchester.db"),
    ProjectDb::version_before(ProjectMigration::LocalDrafts),
  );
  let connection = Connection::open(old.path().join(".orchester").join("orchester.db")).expect("the old database");
  connection
    .execute_batch(
      "INSERT INTO resource_drafts (resource_id, body, based_on, updated_at) \
       SELECT resource_id, body, number, 12 FROM resource_versions AS versions \
       WHERE number = (SELECT MAX(number) FROM resource_versions WHERE resource_id = versions.resource_id);",
    )
    .expect("the drafts");
  connection.execute_batch(prepare).expect("prepared");
  drop(connection);
  let db = ProjectDb::open(old.path()).expect("migrated");
  (old, db)
}

fn has_draft<B: ResourceBody>(db: &ProjectDb, id: ResourceId) -> bool {
  db.draft::<B>(id).expect("read").is_some()
}

#[test]
fn local_drafts_migration_drops_the_drafts_of_published_resources_and_keeps_the_rest() {
  let (repo, seeded) = seeded();
  let (_old, db) = migrated(&repo, "");

  assert!(
    !has_draft::<AgentProfile>(&db, seeded.profile),
    "the profile's draft is gone"
  );
  assert!(
    !has_draft::<Instruction>(&db, seeded.instruction),
    "the instruction's draft is gone"
  );
  assert!(!has_draft::<Skill>(&db, seeded.skill), "the skill's draft is gone");
  assert!(!has_draft::<Tool>(&db, seeded.tool), "the tool's draft is gone");
  assert!(
    has_draft::<Pipeline>(&db, seeded.pipeline),
    "a pipeline keeps its draft"
  );
  assert!(
    has_draft::<Instruction>(&db, seeded.unpublished),
    "a resource never published keeps its draft"
  );
  for (id, published) in [
    (
      seeded.profile,
      db.latest_version::<AgentProfile>(seeded.profile).map(|v| v.is_some()),
    ),
    (
      seeded.instruction,
      db.latest_version::<Instruction>(seeded.instruction)
        .map(|v| v.is_some()),
    ),
    (
      seeded.skill,
      db.latest_version::<Skill>(seeded.skill).map(|v| v.is_some()),
    ),
    (seeded.tool, db.latest_version::<Tool>(seeded.tool).map(|v| v.is_some())),
  ] {
    assert!(published.expect("read"), "{id} keeps its published version");
  }
}

#[test]
fn local_drafts_migration_leaves_a_team_projects_drafts() {
  let (repo, seeded) = seeded();
  let (_old, db) = migrated(
    &repo,
    &format!(
      "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, \
       last_synced_at) VALUES (1, '{server}', '{project}', 'ORC', 'owner', 1, 0, 1);",
      server = ProjectId::generate(),
      project = ProjectId::generate(),
    ),
  );

  assert!(has_draft::<AgentProfile>(&db, seeded.profile));
  assert!(has_draft::<Instruction>(&db, seeded.instruction));
  assert!(has_draft::<Skill>(&db, seeded.skill));
  assert!(has_draft::<Tool>(&db, seeded.tool));
}

#[test]
fn local_drafts_migration_applied_again_changes_nothing() {
  let (repo, seeded) = seeded();
  let (old, db) = migrated(&repo, "");
  drop(db);
  let connection = Connection::open(old.path().join(".orchester").join("orchester.db")).expect("open");

  ProjectDb::apply_migration_for_test(&connection, ProjectMigration::LocalDrafts).expect("applied again");
  drop(connection);

  let db = ProjectDb::open(old.path()).expect("open");
  assert!(!has_draft::<Instruction>(&db, seeded.instruction));
  assert!(has_draft::<Pipeline>(&db, seeded.pipeline));
  assert!(has_draft::<Instruction>(&db, seeded.unpublished));
  assert!(
    db.latest_version::<Instruction>(seeded.instruction)
      .expect("read")
      .is_some()
  );
}
