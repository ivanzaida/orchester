use orchester_core::{
  command::{Command, CommandError},
  connection::{
    AgentConnection, AgentProvider, CheckResult, ConnectionCheck, ConnectionError, ConnectionStatus, GitHubCheck,
    GitHubCheckResult, GitHubConnection, GitHubHost, GitHubLogin, LastCheck, SignIn, check_github,
    gh_auth_status_command, gh_version_command, parse_gh_auth_status, parse_gh_version,
  },
  id::ConnectionId,
  name::Name,
  repository::{GitHubRepo, gh_repo_view_command},
};

use crate::replay::{Replay, failed, fixture, gh, now, ok};

const SIGNED_IN: &str = fixture!("gh_auth_status_signed_in.stdout");

fn host(value: &str) -> GitHubHost {
  GitHubHost::new(value).expect("valid host")
}

fn login(value: &str) -> GitHubLogin {
  GitHubLogin::new(value).expect("valid login")
}

pub fn github_connection() -> GitHubConnection {
  GitHubConnection::new(
    ConnectionId::generate(),
    Name::new("GitHub").expect("valid name"),
    GitHubHost::github_com(),
    gh(),
    now(),
  )
  .expect("absolute executable")
}

#[test]
fn gh_version_is_read_from_captured_output() {
  assert_eq!(
    parse_gh_version(&ok(fixture!("gh_version.stdout"))).as_deref(),
    Some("2.97.0")
  );
  assert_eq!(parse_gh_version(&ok("git version 2.46.0.windows.1")), None);
  assert_eq!(parse_gh_version(&failed("gh version 2.97.0")), None);
}

#[test]
fn gh_auth_status_signed_in() {
  let status = parse_gh_auth_status(&GitHubHost::github_com(), &ok(SIGNED_IN)).expect("readable status");
  assert_eq!(status.sign_in, SignIn::SignedIn);
  assert_eq!(status.account, Some(login("ivanzaida")));
  assert_eq!(status.scopes, ["gist", "read:org", "repo", "workflow"]);
}

#[test]
fn gh_auth_status_not_signed_in() {
  let output = orchester_core::command::CommandOutput {
    code: Some(0),
    stdout: fixture!("gh_auth_status_not_signed_in.stdout").to_owned(),
    stderr: fixture!("gh_auth_status_not_signed_in.stderr").to_owned(),
  };
  let status = parse_gh_auth_status(&GitHubHost::github_com(), &output).expect("readable status");
  assert_eq!(status.sign_in, SignIn::SignedOut);
  assert_eq!(status.account, None);
  assert!(status.scopes.is_empty());
  // Signed in on github.com says nothing about another host.
  let elsewhere = parse_gh_auth_status(&host("ghe.example.com"), &ok(SIGNED_IN)).expect("readable status");
  assert_eq!(elsewhere.sign_in, SignIn::SignedOut);
}

#[test]
fn gh_auth_status_rejected_token_is_signed_out() {
  let status = parse_gh_auth_status(
    &GitHubHost::github_com(),
    &ok(fixture!("gh_auth_status_bad_token.stdout")),
  )
  .expect("readable status");
  assert_eq!(status.sign_in, SignIn::SignedOut);
  assert_eq!(status.account, None);
}

#[test]
fn gh_auth_status_multiple_accounts_and_hosts() {
  // Synthetic: two accounts on github.com (the second active) and a timed-out enterprise host.
  let output = ok(fixture!("gh_auth_status_multiple_accounts.stdout"));
  let status = parse_gh_auth_status(&GitHubHost::github_com(), &output).expect("readable status");
  assert_eq!(status.account, Some(login("ivanzaida")));
  assert_eq!(status.sign_in, SignIn::SignedIn);
  let enterprise = parse_gh_auth_status(&host("GHE.example.com"), &output).expect("readable status");
  assert_eq!(enterprise.account, Some(login("ivan_corp")));
  assert_eq!(enterprise.sign_in, SignIn::Unknown);
}

#[test]
fn gh_auth_status_unrecognized() {
  let github = GitHubHost::github_com();
  assert_eq!(parse_gh_auth_status(&github, &ok("Logged in to github.com")), None);
  assert_eq!(parse_gh_auth_status(&github, &failed("unknown flag: --json")), None);
}

#[test]
fn check_github_reads_version_and_account_with_a_safe_environment() {
  let runner = Replay::new([
    ("--version", Ok(ok(fixture!("gh_version.stdout")))),
    ("auth status --hostname github.com --json hosts", Ok(ok(SIGNED_IN))),
  ]);
  let mut github = github_connection();
  github.record_check(check_github(github.executable(), github.host(), &runner, now()));
  assert_eq!(github.status(), ConnectionStatus::Connected);
  assert_eq!(github.version(), Some("2.97.0"));
  let result = &github.last_check().expect("checked").result;
  assert_eq!(result.signed_in_account(), Some(&login("ivanzaida")));
  for command in runner.ran() {
    assert!(command.env().contains(&("GH_PROMPT_DISABLED".into(), "1".into())));
    assert!(command.env().contains(&("NO_COLOR".into(), "1".into())));
    assert!(command.env_removed().contains(&"GH_DEBUG".to_owned()));
    assert!(command.stdin().is_none());
  }
}

#[test]
fn check_github_failures_become_statuses() {
  let cases = [
    (Err(CommandError::NotFound), ConnectionStatus::ExecutableMissing),
    (Err(CommandError::TimedOut), ConnectionStatus::CouldNotRun),
    (Ok(ok("codex-cli 0.139.0")), ConnectionStatus::NotRecognized),
  ];
  for (answer, expected) in cases {
    let runner = Replay::new([("--version", answer)]);
    let check = check_github(&gh(), &GitHubHost::github_com(), &runner, now());
    assert_eq!(check.result.status(), expected);
    assert_eq!(check.result.signed_in_account(), None);
  }
  let runner = Replay::new([
    ("--version", Ok(ok(fixture!("gh_version.stdout")))),
    (
      "auth status --hostname github.com --json hosts",
      Err(CommandError::TimedOut),
    ),
  ]);
  let check = check_github(&gh(), &GitHubHost::github_com(), &runner, now());
  assert_eq!(check.result.status(), ConnectionStatus::SignInUnknown);
  let runner = Replay::new([
    ("--version", Ok(ok(fixture!("gh_version.stdout")))),
    (
      "auth status --hostname github.com --json hosts",
      Ok(ok(fixture!("gh_auth_status_not_signed_in.stdout"))),
    ),
  ]);
  let check = check_github(&gh(), &GitHubHost::github_com(), &runner, now());
  assert_eq!(check.result.status(), ConnectionStatus::SignInRequired);
}

/// Orchester never signs in or out, switches accounts, refreshes or reveals tokens, configures
/// git credentials or changes gh's configuration, whatever host or repository it is given.
#[test]
fn no_gh_command_can_change_or_reveal_the_sign_in() {
  const FORBIDDEN: [&str; 9] = [
    "token",
    "--show-token",
    "-t",
    "login",
    "logout",
    "switch",
    "refresh",
    "setup-git",
    "set",
  ];
  let hosts = ["github.com", "ghe.example.com", "GitHub.COM"].map(host);
  let mut commands: Vec<Command> = vec![gh_version_command(&gh())];
  for host in &hosts {
    commands.push(gh_auth_status_command(&gh(), host));
    let repo = GitHubRepo::new(host.clone(), "token", "login").expect("valid repository");
    commands.push(gh_repo_view_command(&gh(), &repo));
  }
  for command in &commands {
    for arg in command.args() {
      assert!(!FORBIDDEN.contains(&arg.as_str()), "`{arg}` in {:?}", command.args());
    }
    let config_set = command.args().windows(2).any(|pair| pair == ["config", "set"]);
    assert!(!config_set);
  }
  for value in ["--show-token", "-t", "auth login", "github.com --show-token", ""] {
    assert!(GitHubHost::new(value).is_err(), "`{value}` is a host");
  }
}

#[test]
fn host_and_login_validation() {
  assert_eq!(host("GitHub.com").as_str(), "github.com");
  for invalid in [
    "https://github.com",
    "github.com:443",
    "-github.com",
    "github..com",
    "git hub.com",
  ] {
    assert!(GitHubHost::new(invalid).is_err(), "`{invalid}`");
  }
  for valid in ["octocat", "ivan_corp", "github-actions[bot]"] {
    assert!(GitHubLogin::new(valid).is_ok(), "`{valid}`");
  }
  for invalid in ["", "-octocat", "octo cat", "octo/cat"] {
    assert!(GitHubLogin::new(invalid).is_err(), "`{invalid}`");
  }
}

#[test]
fn connections_are_typed_by_kind() {
  let github = github_connection();
  let id = github.id();
  let general = github.into_connection();
  assert_eq!(
    AgentConnection::try_from(general.clone()),
    Err(ConnectionError::NotAgent(id))
  );
  let mut general = general;
  let agent_check = LastCheck::Agent(ConnectionCheck {
    at: now(),
    result: CheckResult::CouldNotRun,
  });
  assert_eq!(
    general.record_check(agent_check),
    Err(ConnectionError::CheckKindMismatch(id))
  );
  let github_check = LastCheck::GitHub(GitHubCheck {
    at: now(),
    result: GitHubCheckResult::ExecutableMissing,
  });
  general.record_check(github_check).expect("a GitHub check fits");
  assert_eq!(general.status(), ConnectionStatus::ExecutableMissing);

  let codex = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex").expect("valid name"),
    AgentProvider::CodexCli,
    gh().with_file_name("codex.exe"),
    now(),
  )
  .expect("absolute executable");
  let codex_id = codex.id();
  assert_eq!(
    GitHubConnection::try_from(codex.into_connection()),
    Err(ConnectionError::NotGitHub(codex_id))
  );
}
