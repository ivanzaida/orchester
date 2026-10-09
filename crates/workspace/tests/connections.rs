//! Saving connections from the add/edit dialogs, deriving "Used by" from agent profiles, and
//! making the project expect the account `gh` uses now.

use std::{fs, path::Path};

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
use orchester_store::{ProjectDb, StoreError};
use orchester_workspace::{
  agents::data::{ProjectAgents, create_profile},
  connections::{
    data::{ConnectionDraft, ProjectConnections, SaveError, save},
    github::{AccountError, GitHubDraft, expect_active_account, save_github},
    probe::found_copy,
  },
};
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
  let executable = stored.executable().expect("a CLI connection has a program");
  assert!(executable.ends_with("bin/claude.exe") || executable.ends_with("bin\\claude.exe"));
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
  assert!(connections.used_by(claude).all(|used| used.model.as_str() == "opus"));
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

/// An npm launcher (`claude.cmd`) is shown as the Node.js script it starts; a program file is
/// what runs itself.
#[cfg(windows)]
#[test]
fn connections_know_what_their_launcher_scripts_start() {
  use orchester_exec::launch::Via;

  let (repo, mut db) = project_db();
  let script = repo.path().join("node_modules").join("cli.js");
  fs::create_dir_all(script.parent().expect("a folder")).expect("script folder");
  fs::write(&script, "").expect("script");
  let shim = repo.path().join("claude.cmd");
  fs::write(&shim, "@SETLOCAL\r\nnode  \"%~dp0\\node_modules\\cli.js\" %*\r\n").expect("launcher");
  let launcher = ConnectionDraft {
    executable: shim,
    ..draft(&repo, "Claude", signed_out())
  };
  let launched = save(&mut db, launcher, now()).expect("saved");
  let direct = save(&mut db, draft(&repo, "Claude direct", signed_out()), now()).expect("saved");
  let connections = ProjectConnections::load(&db).expect("load");
  let via = |id| connections.launch_of(id).map(|launch| launch.via().clone());
  assert_eq!(via(launched), Some(Via::NodeScript { script }));
  assert_eq!(via(direct), Some(Via::Direct));
}

/// An npm launcher of a program file (Claude Code's `claude.cmd` since 2.1) is shown as that
/// program, which is what starts.
#[cfg(windows)]
#[test]
fn connections_know_the_program_their_launcher_starts() {
  use orchester_exec::launch::Via;

  let (repo, mut db) = project_db();
  let program = repo.path().join("node_modules").join("claude.exe");
  fs::create_dir_all(program.parent().expect("a folder")).expect("program folder");
  fs::write(&program, "").expect("program");
  let shim = repo.path().join("claude.cmd");
  fs::write(&shim, "@SETLOCAL\r\n\"%~dp0\\node_modules\\claude.exe\" %*\r\n").expect("launcher");
  let launcher = ConnectionDraft {
    executable: shim,
    ..draft(&repo, "Claude", signed_out())
  };
  let id = save(&mut db, launcher, now()).expect("saved");
  let connections = ProjectConnections::load(&db).expect("load");
  let launch = connections.launch_of(id).expect("a launch");
  assert_eq!(launch.via(), &Via::LauncherProgram);
  assert_eq!(launch.program(), program);
}

#[test]
fn editing_or_checking_a_connection_keeps_the_versions_its_models_need() {
  let (repo, mut db) = project_db();
  let id = save(&mut db, draft(&repo, "Claude", signed_out()), now()).expect("saved");
  let mut stored =
    orchester_core::connection::AgentConnection::try_from(db.connection(id).expect("read").expect("exists"))
      .expect("an agent connection");
  let opus = ModelId::new("claude-opus-5-5").expect("valid model");
  stored.record_model_requirement(&opus, "2.1.280");
  db.save_connection(&stored).expect("recorded");
  let mut edit = draft(&repo, "Claude · new", signed_out());
  edit.id = Some(id);
  save(&mut db, edit, now()).expect("edited");
  let edited = orchester_core::connection::AgentConnection::try_from(db.connection(id).expect("read").expect("exists"))
    .expect("an agent connection");
  assert_eq!(edited.model_requirements().required(&opus), Some("2.1.280"));
  assert_eq!(
    edited.cannot_run(&opus).map(|too_old| too_old.required),
    Some("2.1.280".to_owned()),
    "2.1.89 is too old"
  );
}

/// `link` made a link to the folder `target`: a junction on Windows (no privilege needed), a symbolic link on
/// macOS.
fn link_folder(target: &Path, link: &Path) {
  #[cfg(windows)]
  {
    let status = std::process::Command::new("cmd")
      .arg("/C")
      .arg("mklink")
      .arg("/J")
      .arg(link)
      .arg(target)
      .output()
      .expect("run mklink");
    assert!(
      status.status.success(),
      "mklink /J: {}",
      String::from_utf8_lossy(&status.stderr)
    );
  }
  #[cfg(unix)]
  std::os::unix::fs::symlink(target, link).expect("a symbolic link");
}

#[test]
fn a_program_named_through_a_link_is_saved_as_the_search_path_names_it() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let bin = folder.path().join("bin");
  fs::create_dir_all(&bin).expect("a folder on the search path");
  let copy = bin.join("codex.exe");
  fs::write(&copy, "").expect("an installed copy");
  let other = folder.path().join("other.exe");
  fs::write(&other, "").expect("another program");
  let linked = folder.path().join("linked");
  link_folder(&bin, &linked);
  let found = vec![copy.clone()];
  assert_eq!(found_copy(&found, &linked.join("codex.exe")), Some(copy.as_path()));
  assert_eq!(found_copy(&found, &copy), Some(copy.as_path()));
  assert_eq!(found_copy(&found, &other), None);
  assert_eq!(found_copy(&found, &folder.path().join("missing.exe")), None);
}
