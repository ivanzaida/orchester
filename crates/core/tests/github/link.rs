use orchester_core::{
  command::{CommandError, CommandOutput},
  connection::{ConnectionStatus, GitHubConnection, GitHubHost, GitHubLogin},
  github::GhCli,
  id::ConnectionId,
  name::Name,
  repository::{RemoteName, RepoPermission, RepositoryError, RepositoryLink, link_repository, parse_git_remotes},
};

use crate::{
  connection::github_connection,
  replay::{Replay, failed, fixture, gh, now, ok},
};

const VERSION: &str = "--version";
const AUTH_STATUS: &str = "auth status --hostname github.com --json hosts";
const VIEW_ORCHESTER: &str =
  "repo view github.com/ivanzaida/orchester --json nameWithOwner,defaultBranchRef,viewerPermission";

fn signed_in(view: Result<CommandOutput, CommandError>) -> Replay {
  Replay::new([
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(fixture!("gh_auth_status_signed_in.stdout")))),
    (VIEW_ORCHESTER, view),
  ])
}

fn link(
  runner: &Replay,
  connection: &GitHubConnection,
  remotes: &str,
  remote: &str,
) -> Result<RepositoryLink, RepositoryError> {
  let remote = RemoteName::new(remote).expect("valid remote name");
  link_repository(
    &GhCli::new(runner, connection),
    connection,
    &parse_git_remotes(remotes),
    &remote,
    now(),
  )
}

fn login(value: &str) -> GitHubLogin {
  GitHubLogin::new(value).expect("valid login")
}

#[test]
fn origin_links_the_repository_for_the_signed_in_account() {
  let connection = github_connection();
  let runner = signed_in(Ok(ok(fixture!("gh_repo_view_orchester.stdout"))));
  let linked = link(
    &runner,
    &connection,
    fixture!("git_remote_orchester.stdout"),
    RemoteName::ORIGIN,
  )
  .expect("linked");
  assert_eq!(linked.connection(), connection.id());
  assert_eq!(linked.remote(), &RemoteName::origin());
  assert_eq!(linked.repo().to_string(), "github.com/ivanzaida/orchester");
  assert_eq!(linked.default_branch().as_str(), "master");
  assert_eq!(linked.permission(), RepoPermission::Admin);
  assert_eq!(linked.expected_login(), &login("ivanzaida"));
  assert_eq!(linked.checked_at(), now());
}

#[test]
fn another_active_account_is_refused() {
  let connection = github_connection();
  let runner = signed_in(Ok(ok(fixture!("gh_repo_view_orchester.stdout"))));
  let linked = link(
    &runner,
    &connection,
    fixture!("git_remote_orchester.stdout"),
    RemoteName::ORIGIN,
  )
  .expect("linked");
  assert_eq!(linked.ensure_account(Some(&login("ivanzaida"))), Ok(()));
  assert_eq!(
    linked.ensure_account(Some(&login("octo-work"))),
    Err(RepositoryError::AccountMismatch {
      expected: login("ivanzaida"),
      active: login("octo-work"),
    })
  );
  assert_eq!(
    linked.ensure_account(None),
    Err(RepositoryError::NotSignedIn(GitHubHost::github_com()))
  );
}

#[test]
fn remotes_that_name_no_repository_on_the_host_are_refused() {
  let connection = github_connection();
  let runner = signed_in(Ok(ok(fixture!("gh_repo_view_orchester.stdout"))));
  assert_eq!(
    link(
      &runner,
      &connection,
      fixture!("git_remote_orchester.stdout"),
      "upstream"
    ),
    Err(RepositoryError::NoGitHubRemote(
      RemoteName::new("upstream").expect("valid")
    ))
  );
  for remote in ["gitlab", "backup"] {
    let refused = link(&runner, &connection, fixture!("git_remote_mixed.stdout"), remote);
    assert!(
      matches!(refused, Err(RepositoryError::NonGitHubRemote { .. })),
      "{remote}: {refused:?}"
    );
  }
  let enterprise = GitHubConnection::new(
    ConnectionId::generate(),
    Name::new("GitHub Enterprise").expect("valid name"),
    GitHubHost::new("ghe.example.com").expect("valid host"),
    gh(),
    now(),
  )
  .expect("absolute executable");
  let refused = link(
    &runner,
    &enterprise,
    fixture!("git_remote_mixed.stdout"),
    RemoteName::ORIGIN,
  )
  .expect_err("origin is on github.com");
  assert!(matches!(refused, RepositoryError::NonGitHubRemote { .. }));
  assert!(!refused.to_string().contains("not-a-real-token"), "{refused}");
  assert!(runner.ran().is_empty(), "gh runs only for a remote on the host");
}

#[test]
fn linking_needs_a_signed_in_gh() {
  let connection = github_connection();
  let signed_out = Replay::new([
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(fixture!("gh_auth_status_bad_token.stdout")))),
  ]);
  assert_eq!(
    link(
      &signed_out,
      &connection,
      fixture!("git_remote_orchester.stdout"),
      RemoteName::ORIGIN
    ),
    Err(RepositoryError::NotSignedIn(GitHubHost::github_com()))
  );
  let missing = Replay::new([(VERSION, Err(CommandError::NotFound))]);
  assert_eq!(
    link(
      &missing,
      &connection,
      fixture!("git_remote_orchester.stdout"),
      RemoteName::ORIGIN
    ),
    Err(RepositoryError::GitHubUnavailable(ConnectionStatus::ExecutableMissing))
  );
}

#[test]
fn repository_errors_are_passed_on() {
  let connection = github_connection();
  let runner = signed_in(Ok(failed(fixture!("gh_repo_view_rate_limited.stderr"))));
  assert_eq!(
    link(
      &runner,
      &connection,
      fixture!("git_remote_orchester.stdout"),
      RemoteName::ORIGIN
    ),
    Err(RepositoryError::RateLimited)
  );
}
