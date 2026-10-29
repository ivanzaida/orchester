//! A profile's approval mode is stored with each of its versions, and profiles stored before it existed read as
//! Manual with their commands as they were (`docs/design/approval-modes.md`).

use std::fs;

use orchester_core::{
  id::{ConnectionId, ProjectId, VersionId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, ApprovalMode, CommandPermission, CommandPrefix, NetworkAccess},
  project::{Project, ProjectKey},
  resource::{AgentProfile, Draft, ModelId, PromptText, ProviderSettings, RepositoryAccess, Slug},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rusqlite::Connection;

fn project_repo() -> (tempfile::TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn profile(commands: CommandPermission, mode: ApprovalMode) -> AgentProfile {
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::Off);
  permissions.set_approval_mode(mode);
  AgentProfile::new(
    Name::new("Builder").expect("valid role"),
    "Implements the plan.".into(),
    PromptText::new("Build it.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("model-a").expect("valid model"),
    ProviderSettings::default(),
  )
  .with_permissions(permissions)
}

fn listed() -> CommandPermission {
  CommandPermission::Allowed(
    AllowedCommands::new(vec![CommandPrefix::new("cargo test").expect("valid command")]).expect("valid list"),
  )
}

#[test]
fn approval_mode_profile_is_stored_with_each_version() {
  let (repo, mut db) = project_repo();
  let manual = profile(listed(), ApprovalMode::Manual);
  let id = db
    .create_resource(
      Slug::new("builder").expect("slug"),
      Name::new("Builder").expect("name"),
      &manual,
      at(1),
    )
    .expect("create")
    .id();
  let v1 = db.publish_draft::<AgentProfile>(id, at(2)).expect("v1");
  let autopilot = profile(listed(), ApprovalMode::Autopilot);
  db.save_draft(id, &Draft::new(autopilot, Some(v1.number()), at(3)))
    .expect("draft");
  db.publish_draft::<AgentProfile>(id, at(4)).expect("v2");
  drop(db);
  let db = ProjectDb::open(repo.path()).expect("reopen");
  let modes: Vec<(ApprovalMode, CommandPermission)> = db
    .versions::<AgentProfile>(id)
    .expect("versions")
    .iter()
    .map(|version| {
      let permissions = version.body().permissions();
      (permissions.approval_mode(), permissions.commands().clone())
    })
    .collect();
  assert_eq!(
    modes,
    [(ApprovalMode::Manual, listed()), (ApprovalMode::Autopilot, listed())]
  );
}

/// Every command permission a profile had before approval modes (schema 4) reads as Manual with that permission
/// kept: *Allowed commands* keeps its list, *Ask before running commands* keeps asking, *Any command in the
/// worktree* stays as it was. Published versions are read as they are, not rewritten.
#[test]
fn approval_mode_profile_schema_4_versions_read_as_manual() {
  for commands in [CommandPermission::AskBefore, listed(), CommandPermission::AnyInWorktree] {
    let (repo, mut db) = project_repo();
    let id = db
      .create_resource(
        Slug::new("builder").expect("slug"),
        Name::new("Builder").expect("name"),
        &profile(commands.clone(), ApprovalMode::Manual),
        at(1),
      )
      .expect("create")
      .id();
    let raw = Connection::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
    let stored: String = raw
      .query_row("SELECT body FROM resource_drafts", [], |row| row.get(0))
      .expect("stored body");
    assert!(stored.contains(r#""schema":5"#), "{stored}");
    let schema_4 = stored.replace(r#""schema":5"#, r#""schema":4"#);
    raw
      .execute(
        "INSERT INTO resource_versions (id, resource_id, number, body, published_at) VALUES (?1, ?2, 1, ?3, 1)",
        rusqlite::params![VersionId::generate().to_string(), id.to_string(), schema_4],
      )
      .expect("store a schema 4 version");
    let version = db.latest_version::<AgentProfile>(id).expect("read").expect("v1");
    assert_eq!(version.body().permissions().approval_mode(), ApprovalMode::Manual);
    assert_eq!(version.body().permissions().commands(), &commands);
    let unchanged: String = raw
      .query_row("SELECT body FROM resource_versions", [], |row| row.get(0))
      .expect("stored body");
    assert_eq!(unchanged, schema_4);
  }
}
