//! Checks and account saves against a local fixture CLI, never the person's GitHub session.
use std::{fs, path::PathBuf, process::Command};

use orchester_core::{
  connection::{GitHubCheck, GitHubCheckResult, GitHubConnection, GitHubHost, GitHubLogin, SignIn},
  id::ConnectionId,
  name::Name,
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryLink, RepositoryView},
  run::BranchName,
  store::{ConnectionStore, RepositoryStore},
  time::Timestamp,
};
use orchester_workspace::tasks::outside::Outside;

use super::{harness::Settings, index::git};

#[test]
fn project_settings_review_github_account_save_refreshes_a_completed_check() {
  let program_dir = fixture_program();
  let mut page = Settings::new(false);
  page.tab("general", "settings-project-name");
  git(&page.repo, &["init"]);
  git(
    &page.repo,
    &["remote", "add", "origin", "https://github.com/example/project.git"],
  );
  link(
    &page,
    program_dir.path().join(format!("gh{}", std::env::consts::EXE_SUFFIX)),
  );
  page.outside(Outside::Connections);
  page.until("wrong account", |p| p.exists("settings-use-account"));
  page.click("settings-github-check");
  page.until("completed repository check", |p| p.exists("settings-github-repo-hint"));
  assert!(page.text("settings-github-login-hint").contains("expected is expected"));
  assert_eq!(page.text("settings-use-account"), "Use signed-in");
  page.click("settings-use-account");
  page.until("account saved and read back", |p| {
    p.exists("settings-github-account")
      && p.text("settings-github-account") == "signed-in"
      && p.text("settings-github-login-hint") == "Signed in as signed-in · matches"
      && !p.exists("settings-use-account")
  });
  page.saved();
  assert_eq!(
    page
      .db()
      .repository_link()
      .expect("link")
      .expect("linked")
      .expected_login()
      .as_str(),
    "signed-in"
  );
}

fn fixture_program() -> tempfile::TempDir {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-9/github");
  fs::create_dir_all(&scratch).expect("scratch folder");
  let dir = tempfile::tempdir_in(&scratch).expect("fixture folder");
  let output = Command::new("rustc")
    .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/project_settings/fixtures/gh.rs"))
    .args(["--edition", "2024", "-o"])
    .arg(dir.path().join(format!("gh{}", std::env::consts::EXE_SUFFIX)))
    .current_dir(dir.path())
    .output()
    .expect("build fixture");
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  dir
}

fn link(page: &Settings, program: PathBuf) {
  let mut db = page.db();
  let mut connection = GitHubConnection::new(
    ConnectionId::generate(),
    Name::new("Fixture GitHub").expect("name"),
    GitHubHost::github_com(),
    program,
    Timestamp::now(),
  )
  .expect("connection");
  connection.record_check(GitHubCheck {
    at: Timestamp::now(),
    result: GitHubCheckResult::Available {
      version: "2.97.0".into(),
      account: Some(GitHubLogin::new("signed-in").expect("login")),
      scopes: vec!["repo".into(), "read:org".into()],
      sign_in: SignIn::SignedIn,
    },
  });
  let view = RepositoryView {
    repo: GitHubRepo::new(GitHubHost::github_com(), "example", "project").expect("repository"),
    default_branch: BranchName::new("main").expect("branch"),
    permission: RepoPermission::Admin,
  };
  let link = RepositoryLink::new(
    connection.id(),
    RemoteName::origin(),
    view,
    GitHubLogin::new("expected").expect("login"),
    Timestamp::now(),
  );
  db.create_connection(&connection.into_connection())
    .expect("save connection");
  db.save_repository_link(&link).expect("save link");
}
