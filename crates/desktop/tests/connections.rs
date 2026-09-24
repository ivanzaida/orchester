//! Saving connections from the add/edit dialog and deriving "Used by" from agent profiles.

use std::fs;

use orchester_core::{
  connection::{AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ConnectionStatus, ModelCatalog, SignIn},
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_desktop::{
  agents::data::create_profile,
  connections::data::{ConnectionDraft, ProjectConnections, SaveError, save},
};
use orchester_store::{ProjectDb, StoreError};
use tempfile::TempDir;

fn project_db() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create the project database");
  (repo, db)
}

fn now() -> Timestamp {
  Timestamp::from_unix_millis(1_700_000_000_000)
}

fn check(result: CheckResult) -> ConnectionCheck {
  ConnectionCheck { at: now(), result }
}

fn signed_out() -> CheckResult {
  CheckResult::Available {
    version: "2.1.89".into(),
    sign_in: SignIn::SignedOut,
    catalog: ModelCatalog::new(CatalogSource::DocumentedAliases, Vec::new()),
  }
}

fn draft(repo: &TempDir, name: &str, result: CheckResult) -> ConnectionDraft {
  ConnectionDraft {
    id: None,
    name: Name::new(name).expect("valid name"),
    provider: AgentProvider::ClaudeCodeCli,
    executable: repo.path().join("claude.exe"),
    check: check(result),
  }
}

#[test]
fn only_verified_connections_are_saved() {
  let (repo, mut db) = project_db();
  for result in [
    CheckResult::ExecutableMissing,
    CheckResult::NotRecognized,
    CheckResult::CouldNotRun,
  ] {
    assert!(matches!(
      save(&mut db, draft(&repo, "Claude", result), now()),
      Err(SaveError::NotVerified)
    ));
  }
  assert!(db.connections().expect("list").is_empty());

  let id = save(&mut db, draft(&repo, "Claude", signed_out()), now()).expect("save");
  let stored = db.connection(id).expect("read").expect("exists");
  assert_eq!(stored.status(), ConnectionStatus::SignInRequired);
}

#[test]
fn editing_a_connection_keeps_its_id_and_rejects_taken_names() {
  let (repo, mut db) = project_db();
  let claude = save(&mut db, draft(&repo, "Claude", signed_out()), now()).expect("save claude");
  save(&mut db, draft(&repo, "Claude 2", signed_out()), now()).expect("save the second");
  let edited = ConnectionDraft {
    id: Some(claude),
    executable: repo.path().join("bin").join("claude.exe"),
    ..draft(&repo, "Claude · Kontur", signed_out())
  };
  assert_eq!(save(&mut db, edited, now()).expect("edit"), claude);
  let stored = db.connection(claude).expect("read").expect("exists");
  assert_eq!(stored.name().as_str(), "Claude · Kontur");
  assert!(stored.executable().ends_with("bin/claude.exe") || stored.executable().ends_with("bin\\claude.exe"));
  let taken = ConnectionDraft {
    id: Some(claude),
    ..draft(&repo, "Claude 2", signed_out())
  };
  assert!(matches!(
    save(&mut db, taken, now()),
    Err(SaveError::Store(StoreError::DuplicateConnectionName(_)))
  ));
}

#[test]
fn used_by_lists_the_profiles_on_each_connection() {
  let (repo, mut db) = project_db();
  let claude = save(&mut db, draft(&repo, "Claude", signed_out()), now()).expect("save");
  let other = save(&mut db, draft(&repo, "Other", signed_out()), now()).expect("save");
  let profile = |role: &str, connection| {
    AgentProfile::new(
      Name::new(role).expect("valid role"),
      String::new(),
      PromptText::new("").expect("valid prompt"),
      connection,
      ModelId::new("opus").expect("valid model"),
      ProviderSettings::default(),
    )
  };
  create_profile(&mut db, &profile("Explorer", claude), now()).expect("explorer");
  create_profile(&mut db, &profile("Reviewer", claude), now()).expect("reviewer");
  create_profile(&mut db, &profile("Builder", other), now()).expect("builder");

  let connections = ProjectConnections::load(&db).expect("load");
  let roles: Vec<&str> = connections.used_by(claude).map(|used| &*used.role).collect();
  assert_eq!(roles, ["Explorer", "Reviewer"]);
  assert!(connections.used_by(claude).all(|used| &*used.model == "opus"));
  assert_eq!(connections.used_by(other).count(), 1);
}
