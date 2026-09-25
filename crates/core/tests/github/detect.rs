use std::path::Path;

use orchester_core::{
  command::{CommandError, CommandOutput},
  connection::{GitHubHost, GitHubLogin},
  id::ConnectionId,
  repository::{DetectRequest, Detection, DetectionState, GhProgram, RemoteName, detect_repository},
};

use crate::replay::{Replay, failed, fixture, gh, now, ok};

const REMOTES: &str = "remote -v";
const VERSION: &str = "--version";
const AUTH_STATUS: &str = "auth status --hostname github.com --json hosts";
const VIEW: &str = "repo view github.com/ivanzaida/orchester --json nameWithOwner,defaultBranchRef,viewerPermission";
const SCOPE_WITHOUT_REPO: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","tokenSource":"keyring","scopes":"read:org","gitProtocol":"https"}]}}"#;
const NOT_A_REPOSITORY: &str = "fatal: not a git repository (or any of the parent directories): .git";

type Answer = Result<CommandOutput, CommandError>;

fn login(value: &str) -> GitHubLogin {
  GitHubLogin::new(value).expect("valid login")
}

fn github_com(found: bool) -> Vec<GhProgram> {
  vec![GhProgram {
    host: GitHubHost::github_com(),
    executable: found.then(gh),
  }]
}

fn detect(runner: &Replay, programs: &[GhProgram]) -> Detection {
  let request = DetectRequest {
    repo_root: Path::new("."),
    git: Path::new("git"),
    remote: &RemoteName::origin(),
    programs,
  };
  detect_repository(runner, &request, now())
}

/// The origin remote on github.com, `gh` signed in as ivanzaida, and `view` for the repository.
fn signed_in(auth: &'static str, view: Answer) -> Replay {
  Replay::new([
    (REMOTES, Ok(ok(fixture!("git_remote_orchester.stdout")))),
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(auth))),
    (VIEW, view),
  ])
}

fn detected() -> Replay {
  signed_in(
    fixture!("gh_auth_status_signed_in.stdout"),
    Ok(ok(fixture!("gh_repo_view_orchester.stdout"))),
  )
}

#[test]
fn detect_finds_the_repository_and_the_signed_in_account() {
  let runner = detected();
  let detection = detect(&runner, &github_com(true));
  assert_eq!(
    detection.state(None),
    DetectionState::Detected {
      account: login("ivanzaida")
    }
  );
  let found = detection.github().expect("a GitHub remote");
  assert_eq!(found.repo().to_string(), "github.com/ivanzaida/orchester");
  assert_eq!(found.version(), Some("2.97.0"));
  let view = found.view().expect("the account reads the repository");
  assert_eq!(view.default_branch.as_str(), "master");
  // Only read-only commands ran: the remotes, gh's version, its status and the repository.
  let ran: Vec<String> = runner.ran().iter().map(|command| command.args().join(" ")).collect();
  assert_eq!(ran, [REMOTES, VERSION, AUTH_STATUS, VIEW]);
}

#[test]
fn detect_links_with_the_active_account_unless_one_is_expected() {
  let detection = detect(&detected(), &github_com(true));
  let connection = ConnectionId::generate();
  let link = detection.link(connection, None, now()).expect("linkable");
  assert_eq!(link.expected_login(), &login("ivanzaida"));
  assert_eq!(link.connection(), connection);
  assert_eq!(link.remote(), &RemoteName::origin());
  let pinned = detection
    .link(connection, Some(&login("ivan-work")), now())
    .expect("linkable");
  assert_eq!(pinned.expected_login(), &login("ivan-work"));
}

#[test]
fn detect_reports_another_account_than_the_expected_one() {
  let detection = detect(&detected(), &github_com(true));
  assert_eq!(
    detection.state(Some(&login("ivan-work"))),
    DetectionState::WrongAccount {
      active: login("ivanzaida"),
      expected: login("ivan-work"),
    }
  );
}

#[test]
fn detect_reports_a_folder_that_is_not_a_repository() {
  let runner = Replay::new([(REMOTES, Ok(failed(NOT_A_REPOSITORY)))]);
  let state = detect(&runner, &github_com(true)).state(None);
  assert_eq!(state, DetectionState::NotAGitRepository);
  assert!(state.blocks_project());
}

#[test]
fn detect_never_blocks_the_project_for_github_states() {
  let states = [
    DetectionState::NoRemote,
    DetectionState::NotGitHub,
    DetectionState::GhNotFound,
    DetectionState::NotSignedIn,
    DetectionState::Unrecognized,
    DetectionState::GitFailed,
  ];
  assert!(states.iter().all(|state| !state.blocks_project()));
}

#[test]
fn detect_reports_a_missing_git_program() {
  let runner = Replay::new([(REMOTES, Err(CommandError::NotFound))]);
  assert_eq!(
    detect(&runner, &github_com(true)).state(None),
    DetectionState::GitFailed
  );
}

#[test]
fn detect_reports_a_repository_without_origin() {
  let runner = Replay::new([(REMOTES, Ok(ok("")))]);
  let detection = detect(&runner, &github_com(true));
  assert_eq!(detection, Detection::NoRemote(RemoteName::origin()));
  assert_eq!(detection.state(None), DetectionState::NoRemote);
}

#[test]
fn detect_reports_a_remote_on_another_host() {
  let runner = Replay::new([(
    REMOTES,
    Ok(ok(
      "origin\thttps://gitlab.example.com/ivan/kontur.git (fetch)\norigin\thttps://gitlab.example.com/ivan/kontur.git (push)\n",
    )),
  )]);
  let detection = detect(&runner, &github_com(true));
  assert_eq!(detection.state(None), DetectionState::NotGitHub);
  assert_eq!(
    detection.remote().map(|remote| remote.url()),
    Some("https://gitlab.example.com/ivan/kontur.git")
  );
}

#[test]
fn detect_strips_credentials_from_the_remote() {
  let runner = Replay::new([
    (REMOTES, Ok(ok(fixture!("git_remote_mixed.stdout")))),
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(fixture!("gh_auth_status_signed_in.stdout")))),
    (VIEW, Ok(ok(fixture!("gh_repo_view_orchester.stdout")))),
  ]);
  let detection = detect(&runner, &github_com(true));
  let url = detection.remote().expect("origin").url();
  assert!(!url.contains("not-a-real-token"), "{url}");
  assert!(matches!(detection.state(None), DetectionState::Detected { .. }));
}

#[test]
fn detect_reports_gh_not_found() {
  let runner = Replay::new([(REMOTES, Ok(ok(fixture!("git_remote_orchester.stdout"))))]);
  let detection = detect(&runner, &github_com(false));
  assert_eq!(detection.state(None), DetectionState::GhNotFound);
  assert!(detection.link(ConnectionId::generate(), None, now()).is_none());

  let missing = Replay::new([
    (REMOTES, Ok(ok(fixture!("git_remote_orchester.stdout")))),
    (VERSION, Err(CommandError::NotFound)),
  ]);
  assert_eq!(
    detect(&missing, &github_com(true)).state(None),
    DetectionState::GhNotFound
  );
}

#[test]
fn detect_reports_gh_not_signed_in() {
  let runner = Replay::new([
    (REMOTES, Ok(ok(fixture!("git_remote_orchester.stdout")))),
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(fixture!("gh_auth_status_not_signed_in.stdout")))),
  ]);
  assert_eq!(
    detect(&runner, &github_com(true)).state(None),
    DetectionState::NotSignedIn
  );
}

#[test]
fn detect_reports_a_repository_the_account_cannot_read() {
  let runner = signed_in(
    fixture!("gh_auth_status_signed_in.stdout"),
    Ok(failed(fixture!("gh_repo_view_not_found.stderr"))),
  );
  let detection = detect(&runner, &github_com(true));
  assert_eq!(
    detection.state(None),
    DetectionState::NoAccess {
      account: login("ivanzaida")
    }
  );
  assert!(detection.link(ConnectionId::generate(), None, now()).is_none());
}

#[test]
fn detect_reports_unrecognized_gh_output() {
  let runner = Replay::new([
    (REMOTES, Ok(ok(fixture!("git_remote_orchester.stdout")))),
    (VERSION, Ok(ok("something else 1.0"))),
  ]);
  assert_eq!(
    detect(&runner, &github_com(true)).state(None),
    DetectionState::Unrecognized
  );
  let view = signed_in(fixture!("gh_auth_status_signed_in.stdout"), Ok(ok("<html>")));
  assert_eq!(
    detect(&view, &github_com(true)).state(None),
    DetectionState::Unrecognized
  );
}

#[test]
fn detect_reports_a_missing_repo_scope() {
  let runner = signed_in(SCOPE_WITHOUT_REPO, Ok(ok(fixture!("gh_repo_view_orchester.stdout"))));
  let detection = detect(&runner, &github_com(true));
  assert_eq!(
    detection.state(None),
    DetectionState::MissingScope {
      account: login("ivanzaida")
    }
  );
}

#[test]
fn detect_reports_rate_limits_and_network_failures() {
  let limited = signed_in(
    fixture!("gh_auth_status_signed_in.stdout"),
    Ok(failed(fixture!("gh_repo_view_rate_limited.stderr"))),
  );
  assert_eq!(
    detect(&limited, &github_com(true)).state(None),
    DetectionState::RateLimited
  );
  let offline = signed_in(
    fixture!("gh_auth_status_signed_in.stdout"),
    Ok(failed(fixture!("gh_repo_view_network.stderr"))),
  );
  assert_eq!(
    detect(&offline, &github_com(true)).state(None),
    DetectionState::Unreachable
  );
}
