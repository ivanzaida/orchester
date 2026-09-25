//! Saving connections from the add/edit dialogs, deriving "Used by" from agent profiles, and
//! making the project expect the account `gh` uses now.

use std::fs;

use orchester_core::{
  connection::{
    AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ConnectionStatus, GitHubCheck, GitHubCheckResult,
    GitHubHost, GitHubLogin, ModelCatalog, SignIn,
  },
  id::{ConnectionId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryLink, RepositoryView},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings},
  run::BranchName,
  store::{ConnectionStore, RepositoryStore},
  time::Timestamp,
};
use orchester_desktop::{
  agents::data::{ProjectAgents, create_profile},
  connections::{
    data::{ConnectionDraft, ProjectConnections, SaveError, save},
    github::{AccountError, GitHubDraft, expect_active_account, save_github},
  },
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

fn login(value: &str) -> GitHubLogin {
  GitHubLogin::new(value).expect("valid login")
}

fn gh_signed_in(account: &str) -> GitHubCheckResult {
  GitHubCheckResult::Available {
    version: "2.97.0".into(),
    account: Some(login(account)),
    scopes: vec!["repo".into()],
    sign_in: SignIn::SignedIn,
  }
}

fn github_draft(repo: &TempDir, result: GitHubCheckResult) -> GitHubDraft {
  GitHubDraft {
    id: None,
    name: Name::new("GitHub · github.com").expect("valid name"),
    host: GitHubHost::github_com(),
    executable: repo.path().join("gh.exe"),
    check: GitHubCheck { at: now(), result },
  }
}

fn link_through(db: &mut ProjectDb, connection: ConnectionId, expected: &str) {
  let view = RepositoryView {
    repo: GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur").expect("valid repository"),
    default_branch: BranchName::new("main").expect("valid branch"),
    permission: RepoPermission::Write,
  };
  let link = RepositoryLink::new(connection, RemoteName::origin(), view, login(expected), now());
  db.save_repository_link(&link).expect("the link is saved");
}

#[test]
fn github_connection_is_saved_once_gh_answers() {
  let (repo, mut db) = project_db();
  for result in [
    GitHubCheckResult::ExecutableMissing,
    GitHubCheckResult::NotRecognized,
    GitHubCheckResult::CouldNotRun,
  ] {
    assert!(matches!(
      save_github(&mut db, github_draft(&repo, result), now()),
      Err(SaveError::NotVerified)
    ));
  }
  let signed_out = GitHubCheckResult::Available {
    version: "2.97.0".into(),
    account: None,
    scopes: Vec::new(),
    sign_in: SignIn::SignedOut,
  };
  let id = save_github(&mut db, github_draft(&repo, signed_out), now()).expect("save");
  let connections = ProjectConnections::load(&db).expect("load");
  let github = connections.github_connection(id).expect("listed as GitHub");
  assert_eq!(github.host(), &GitHubHost::github_com());
  assert_eq!(github.status(), ConnectionStatus::SignInRequired);
  assert!(connections.connections.is_empty());
  assert!(connections.link_of(id).is_none());
}

#[test]
fn github_connections_are_not_offered_to_agent_profiles() {
  let (repo, mut db) = project_db();
  save_github(&mut db, github_draft(&repo, gh_signed_in("ivanzaida")), now()).expect("save github");
  let claude = save(&mut db, draft(&repo, "Claude", signed_out()), now()).expect("save claude");
  let agents = ProjectAgents::load(&db).expect("load");
  let offered: Vec<ConnectionId> = agents.connections.iter().map(|connection| connection.id()).collect();
  assert_eq!(offered, [claude]);
}

#[test]
fn github_use_this_account_records_the_account_gh_uses_now() {
  let (repo, mut db) = project_db();
  let id = save_github(&mut db, github_draft(&repo, gh_signed_in("ivanzaida")), now()).expect("save");
  // Nothing to change without a link through the connection.
  let fresh = GitHubCheck {
    at: now(),
    result: gh_signed_in("ivan-work"),
  };
  assert!(matches!(
    expect_active_account(&mut db, id, fresh.clone()),
    Err(AccountError::NotLinked(_))
  ));
  link_through(&mut db, id, "ivanzaida");
  expect_active_account(&mut db, id, fresh).expect("recorded");
  let link = db.repository_link().expect("read").expect("linked");
  assert_eq!(link.expected_login(), &login("ivan-work"));
  // The fresh check is stored with it.
  let connections = ProjectConnections::load(&db).expect("load");
  let github = connections.github_connection(id).expect("listed");
  assert_eq!(
    github.last_check().and_then(|check| check.result.signed_in_account()),
    Some(&login("ivan-work"))
  );
  // Signed out now: the expected account stays, and the check is still stored.
  let signed_out = GitHubCheck {
    at: now(),
    result: GitHubCheckResult::Available {
      version: "2.97.0".into(),
      account: None,
      scopes: Vec::new(),
      sign_in: SignIn::SignedOut,
    },
  };
  assert!(matches!(
    expect_active_account(&mut db, id, signed_out),
    Err(AccountError::Repository(_))
  ));
  let link = db.repository_link().expect("read").expect("linked");
  assert_eq!(link.expected_login(), &login("ivan-work"));
  let stored = db.connection(id).expect("read").expect("stored");
  assert_eq!(stored.status(), ConnectionStatus::SignInRequired);
}
