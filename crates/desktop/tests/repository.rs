//! The detected GitHub repository on the desktop: what Create may do in each detection state,
//! what a new project takes over from detection (the GitHub connection and the link with its
//! expected account), what Check again records for a linked project, and the "Used by" links
//! that follow. Detection runs against captured `git` and `gh` output replayed by a fake runner.

use std::{cell::RefCell, collections::HashMap, fs, path::Path};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{ConnectionStatus, GitHubHost, GitHubLogin},
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  repository::{DetectRequest, Detection, DetectionState, GhProgram, RemoteName, detect_repository},
  store::{ConnectionStore, RepositoryStore},
  time::Timestamp,
};
use orchester_desktop::{
  connections::{data::ProjectConnections, github::expect_active_account},
  project_setup::{ProjectDraft, creatable},
  repository::{adopt, link, refresh},
};
use orchester_store::ProjectDb;
use tempfile::TempDir;

const REMOTES: &str = "remote -v";
const VERSION: &str = "--version";
const AUTH_STATUS: &str = "auth status --hostname github.com --json hosts";
const VIEW: &str = "repo view github.com/ivanzaida/orchester --json nameWithOwner,defaultBranchRef,viewerPermission";
const ORIGIN: &str = "origin\thttps://github.com/ivanzaida/orchester.git (fetch)\norigin\thttps://github.com/ivanzaida/orchester.git (push)\n";
const GH_VERSION: &str = "gh version 2.97.0 (2026-07-31)\nhttps://github.com/cli/cli/releases/tag/v2.97.0\n";
const VIEW_OUTPUT: &str =
  r#"{"defaultBranchRef":{"name":"master"},"nameWithOwner":"ivanzaida/orchester","viewerPermission":"ADMIN"}"#;
const SIGNED_OUT: &str = r#"{"hosts":{}}"#;
const NAME: &str = "GitHub · github.com";

/// Answers each argument list with a captured output; anything else was never allowed to run.
struct Replay(RefCell<HashMap<String, Result<CommandOutput, CommandError>>>);

impl Replay {
  fn new(answers: impl IntoIterator<Item = (&'static str, Result<CommandOutput, CommandError>)>) -> Self {
    Self(RefCell::new(
      answers
        .into_iter()
        .map(|(args, answer)| (args.to_owned(), answer))
        .collect(),
    ))
  }
}

impl CommandRunner for Replay {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let key = command.args().join(" ");
    self
      .0
      .borrow()
      .get(&key)
      .cloned()
      .unwrap_or_else(|| panic!("`{key}` is not one of the expected read-only commands"))
  }
}

fn ok(stdout: &str) -> Result<CommandOutput, CommandError> {
  Ok(CommandOutput {
    success: true,
    stdout: stdout.to_owned(),
    stderr: String::new(),
  })
}

fn status(login: &str) -> String {
  format!(
    r#"{{"hosts":{{"github.com":[{{"state":"success","active":true,"host":"github.com","login":"{login}","tokenSource":"keyring","scopes":"gist, read:org, repo, workflow","gitProtocol":"https"}}]}}}}"#
  )
}

fn now() -> Timestamp {
  Timestamp::from_unix_millis(1_700_000_000_000)
}

fn login(value: &str) -> GitHubLogin {
  GitHubLogin::new(value).expect("valid login")
}

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

/// Detection in `repo` with `gh` at `repo/gh.exe` answering as `runner` does.
fn detect(repo: &Path, runner: &Replay) -> Detection {
  let programs = [GhProgram {
    host: GitHubHost::github_com(),
    executable: Some(repo.join("gh.exe")),
  }];
  let request = DetectRequest {
    repo_root: repo,
    git: Path::new("git"),
    remote: &RemoteName::origin(),
    programs: &programs,
  };
  detect_repository(runner, &request, now())
}

fn signed_in_as(account: &str) -> Replay {
  Replay::new([
    (REMOTES, ok(ORIGIN)),
    (VERSION, ok(GH_VERSION)),
    (AUTH_STATUS, ok(&status(account))),
    (VIEW, ok(VIEW_OUTPUT)),
  ])
}

#[test]
fn repository_create_is_blocked_only_by_a_folder_outside_git() {
  let draft = || {
    ProjectDraft {
      name: "Kontur",
      repository: Some(Path::new("C:/work/kontur")),
    }
    .validate()
  };
  let account = || login("ivanzaida");
  let github_states = [
    DetectionState::Detected { account: account() },
    DetectionState::NoRemote,
    DetectionState::NotGitHub,
    DetectionState::GhNotFound,
    DetectionState::NotSignedIn,
    DetectionState::WrongAccount {
      active: login("ivan-work"),
      expected: account(),
    },
    DetectionState::NoAccess { account: account() },
    DetectionState::Unrecognized,
    DetectionState::MissingScope { account: account() },
    DetectionState::GitFailed,
    DetectionState::CouldNotRun,
    DetectionState::Unreachable,
    DetectionState::RateLimited,
    DetectionState::NoDefaultBranch,
  ];
  for state in &github_states {
    assert!(creatable(draft(), Some(state)).is_some(), "{state:?} blocks Create");
  }
  // Still detecting: Create does not wait for GitHub.
  assert!(creatable(draft(), None).is_some());
  assert!(creatable(draft(), Some(&DetectionState::NotAGitRepository)).is_none());
  // Without a folder, Create stays disabled whatever detection says.
  let no_folder = ProjectDraft {
    name: "Kontur",
    repository: None,
  }
  .validate();
  assert!(creatable(no_folder, None).is_none());
}

#[test]
fn repository_create_links_with_the_account_gh_used_at_detection() {
  let (repo, mut db) = project_db();
  let detection = detect(repo.path(), &signed_in_as("ivanzaida"));
  let linked = adopt(&mut db, &detection, None, NAME, now())
    .expect("recorded")
    .expect("linked");
  assert_eq!(linked.expected_login(), &login("ivanzaida"));
  assert_eq!(linked.repo().to_string(), "github.com/ivanzaida/orchester");
  assert_eq!(linked.default_branch().as_str(), "master");
  assert_eq!(db.repository_link().expect("read"), Some(linked.clone()));
  // The GitHub connection for the host is added with detection's check, and the inspector's
  // "Used by" block and the row's repository count follow the link.
  let connections = ProjectConnections::load(&db).expect("load");
  let [github] = connections.github.as_slice() else {
    panic!("one GitHub connection, got {:?}", connections.github);
  };
  assert_eq!(github.name().as_str(), NAME);
  assert_eq!(github.host(), &GitHubHost::github_com());
  assert_eq!(github.executable(), repo.path().join("gh.exe"));
  assert_eq!(github.status(), ConnectionStatus::Connected);
  assert_eq!(linked.connection(), github.id());
  assert_eq!(connections.link_of(github.id()), Some(&linked));
}

#[test]
fn repository_create_keeps_the_account_the_project_was_set_to_expect() {
  let (repo, mut db) = project_db();
  let detection = detect(repo.path(), &signed_in_as("ivanzaida"));
  // Use this account in setup made the project expect ivan-work before Create.
  let linked = adopt(&mut db, &detection, Some(&login("ivan-work")), NAME, now())
    .expect("recorded")
    .expect("linked");
  assert_eq!(linked.expected_login(), &login("ivan-work"));
}

#[test]
fn repository_create_without_sign_in_adds_only_the_connection() {
  let (repo, mut db) = project_db();
  let runner = Replay::new([
    (REMOTES, ok(ORIGIN)),
    (VERSION, ok(GH_VERSION)),
    (AUTH_STATUS, ok(SIGNED_OUT)),
  ]);
  let detection = detect(repo.path(), &runner);
  assert_eq!(detection.state(None), DetectionState::NotSignedIn);
  assert_eq!(adopt(&mut db, &detection, None, NAME, now()).expect("recorded"), None);
  assert!(db.repository_link().expect("read").is_none());
  let connections = db.connections().expect("list");
  assert_eq!(connections.len(), 1);
  assert_eq!(connections[0].status(), ConnectionStatus::SignInRequired);
}

#[test]
fn repository_create_without_gh_or_github_remote_records_nothing() {
  let (repo, mut db) = project_db();
  let no_gh = Replay::new([(REMOTES, ok(ORIGIN)), (VERSION, Err(CommandError::NotFound))]);
  let local = Replay::new([(REMOTES, ok(""))]);
  for runner in [no_gh, local] {
    let detection = detect(repo.path(), &runner);
    assert_eq!(adopt(&mut db, &detection, None, NAME, now()).expect("recorded"), None);
  }
  assert!(db.connections().expect("list").is_empty());
  assert!(db.repository_link().expect("read").is_none());
}

#[test]
fn repository_link_later_needs_a_readable_repository() {
  let (repo, mut db) = project_db();
  let runner = Replay::new([
    (REMOTES, ok(ORIGIN)),
    (VERSION, ok(GH_VERSION)),
    (AUTH_STATUS, ok(SIGNED_OUT)),
  ]);
  let detection = detect(repo.path(), &runner);
  assert!(link(&mut db, &detection, None, NAME, now()).is_err());
  let detection = detect(repo.path(), &signed_in_as("ivanzaida"));
  let linked = link(&mut db, &detection, None, NAME, now()).expect("linked");
  assert_eq!(linked.expected_login(), &login("ivanzaida"));
  // The connection added by the failed attempt is reused, not added twice.
  assert_eq!(db.connections().expect("list").len(), 1);
}

#[test]
fn repository_check_again_keeps_the_expected_account() {
  let (repo, mut db) = project_db();
  let first = detect(repo.path(), &signed_in_as("ivanzaida"));
  adopt(&mut db, &first, None, NAME, now()).expect("recorded");
  // gh switched accounts in a terminal; checking again records it, the project still expects
  // ivanzaida, and the settings read-back shows the wrong account.
  let later = Timestamp::from_unix_millis(1_700_000_100_000);
  let switched = detect(repo.path(), &signed_in_as("ivan-work"));
  assert_eq!(
    switched.state(Some(&login("ivanzaida"))),
    DetectionState::WrongAccount {
      active: login("ivan-work"),
      expected: login("ivanzaida"),
    }
  );
  refresh(&mut db, &switched, NAME, later).expect("recorded");
  let link = db.repository_link().expect("read").expect("linked");
  assert_eq!(link.expected_login(), &login("ivanzaida"));
  assert_eq!(link.checked_at(), later);
  let github = ProjectConnections::load(&db).expect("load").github[0].clone();
  assert_eq!(
    github.last_check().and_then(|check| check.result.signed_in_account()),
    Some(&login("ivan-work"))
  );
  // Use signed-in account: the project now expects the account gh uses.
  let check = github.last_check().expect("checked").clone();
  expect_active_account(&mut db, github.id(), check).expect("recorded");
  let link = db.repository_link().expect("read").expect("linked");
  assert_eq!(link.expected_login(), &login("ivan-work"));
}

#[test]
fn repository_check_again_of_an_unlinked_project_records_nothing() {
  let (repo, mut db) = project_db();
  let detection = detect(repo.path(), &signed_in_as("ivanzaida"));
  refresh(&mut db, &detection, NAME, now()).expect("nothing to record");
  assert!(db.connections().expect("list").is_empty());
  assert!(db.repository_link().expect("read").is_none());
}
