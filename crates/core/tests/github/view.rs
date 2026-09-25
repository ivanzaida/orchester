use orchester_core::{
  command::CommandError,
  connection::GitHubHost,
  github::{GhCli, GitHub},
  repository::{GitHubRepo, RepoPermission, RepositoryError, parse_gh_repo_view},
};

use crate::replay::{Replay, failed, fixture, ok};

fn repo(owner: &str, name: &str) -> GitHubRepo {
  GitHubRepo::new(GitHubHost::github_com(), owner, name).expect("valid repository")
}

#[test]
fn repository_view_is_read_from_captured_output() {
  let view = parse_gh_repo_view(
    &repo("ivanzaida", "orchester"),
    &ok(fixture!("gh_repo_view_orchester.stdout")),
  )
  .expect("a readable view");
  assert_eq!(view.repo.to_string(), "github.com/ivanzaida/orchester");
  assert_eq!(view.default_branch.as_str(), "master");
  assert_eq!(view.permission, RepoPermission::Admin);
  assert!(view.permission.can_push());

  let linux = parse_gh_repo_view(
    &repo("torvalds", "linux"),
    &ok(fixture!("gh_repo_view_read_only.stdout")),
  )
  .expect("a readable view");
  assert_eq!(linux.permission, RepoPermission::Read);
  assert!(!linux.permission.can_push());
}

#[test]
fn renamed_repositories_are_reported_under_their_current_name() {
  let view = parse_gh_repo_view(
    &repo("ivanzaida", "old-name"),
    &ok(fixture!("gh_repo_view_lurq.stdout")),
  )
  .expect("a readable view");
  assert_eq!(view.repo, repo("ivanzaida", "lurq"));
}

#[test]
fn repository_view_failures_are_classified() {
  let requested = repo("ivanzaida", "does-not-exist-orch-xyz");
  let cases = [
    (
      fixture!("gh_repo_view_not_found.stderr"),
      RepositoryError::NotFound(requested.clone()),
    ),
    (
      fixture!("gh_repo_view_not_signed_in.stderr"),
      RepositoryError::NotSignedIn(GitHubHost::github_com()),
    ),
    (
      fixture!("gh_repo_view_bad_credentials.stderr"),
      RepositoryError::NotSignedIn(GitHubHost::github_com()),
    ),
    // Synthetic: the message gh prints for an exhausted GraphQL rate limit.
    (
      fixture!("gh_repo_view_rate_limited.stderr"),
      RepositoryError::RateLimited,
    ),
    (
      "HTTP 403: Resource protected by organization SAML enforcement.",
      RepositoryError::NoAccess(requested.clone()),
    ),
  ];
  for (stderr, expected) in cases {
    assert_eq!(
      parse_gh_repo_view(&requested, &failed(stderr)),
      Err(expected),
      "{stderr}"
    );
  }
  let network = parse_gh_repo_view(&requested, &failed(fixture!("gh_repo_view_network.stderr")));
  assert!(matches!(network, Err(RepositoryError::Network { .. })), "{network:?}");
  let unknown = parse_gh_repo_view(&requested, &failed("panic at https://u:not-a-real-token@x.io/"));
  let Err(RepositoryError::Unrecognized { program: "gh", detail }) = unknown else {
    panic!("expected an unrecognized answer, got {unknown:?}");
  };
  assert!(!detail.contains("not-a-real-token"), "{detail}");
}

#[test]
fn repository_view_answers_that_cannot_be_linked() {
  let requested = repo("ivanzaida", "empty");
  let empty = r#"{"defaultBranchRef":{"name":""},"nameWithOwner":"ivanzaida/empty","viewerPermission":"WRITE"}"#;
  assert_eq!(
    parse_gh_repo_view(&requested, &ok(empty)),
    Err(RepositoryError::NoDefaultBranch(requested.clone()))
  );
  let no_access = r#"{"defaultBranchRef":{"name":"main"},"nameWithOwner":"ivanzaida/empty","viewerPermission":null}"#;
  assert_eq!(
    parse_gh_repo_view(&requested, &ok(no_access)),
    Err(RepositoryError::NoAccess(requested.clone()))
  );
  assert!(matches!(
    parse_gh_repo_view(&requested, &ok("not json")),
    Err(RepositoryError::Unrecognized { .. })
  ));
}

#[test]
fn gh_cli_views_repositories_on_its_host_only() {
  let runner = Replay::new([(
    "repo view github.com/ivanzaida/orchester --json nameWithOwner,defaultBranchRef,viewerPermission",
    Ok(ok(fixture!("gh_repo_view_orchester.stdout"))),
  )]);
  let connection = super::connection::github_connection();
  let github = GhCli::new(&runner, &connection);
  let view = github.repository(&repo("ivanzaida", "orchester")).expect("view");
  assert_eq!(view.permission, RepoPermission::Admin);

  let enterprise = GitHubRepo::new(GitHubHost::new("ghe.example.com").expect("host"), "o", "r").expect("repo");
  assert!(matches!(
    github.repository(&enterprise),
    Err(RepositoryError::OtherHost { .. })
  ));
  let broken = Replay::new([(
    "repo view github.com/ivanzaida/orchester --json nameWithOwner,defaultBranchRef,viewerPermission",
    Err(CommandError::TimedOut),
  )]);
  assert!(matches!(
    GhCli::new(&broken, &connection).repository(&repo("ivanzaida", "orchester")),
    Err(RepositoryError::Run {
      program: "gh",
      source: CommandError::TimedOut
    })
  ));
}
