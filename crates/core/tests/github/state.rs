use orchester_core::{
  connection::{GhAdvice, GitHubCheckResult, GitHubHost, GitHubLogin, GitHubState, SignIn},
  id::ConnectionId,
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryError, RepositoryLink, RepositoryView},
  run::BranchName,
};

use crate::replay::now;

fn login(value: &str) -> GitHubLogin {
  GitHubLogin::new(value).expect("valid login")
}

fn signed_in(account: &str, scopes: &[&str]) -> GitHubCheckResult {
  GitHubCheckResult::Available {
    version: "2.97.0".into(),
    account: Some(login(account)),
    scopes: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
    sign_in: SignIn::SignedIn,
  }
}

#[test]
fn state_follows_the_check_and_the_expected_account() {
  let full = signed_in("ivanzaida", &["read:org", "repo"]);
  assert_eq!(GitHubState::of(None, None), GitHubState::NotChecked);
  assert_eq!(
    GitHubState::of(Some(&full), None),
    GitHubState::SignedIn {
      account: login("ivanzaida")
    }
  );
  assert_eq!(
    GitHubState::of(Some(&full), Some(&login("ivan-work"))),
    GitHubState::WrongAccount {
      active: login("ivanzaida"),
      expected: login("ivan-work")
    }
  );
  let signed_out = GitHubCheckResult::Available {
    version: "2.97.0".into(),
    account: None,
    scopes: Vec::new(),
    sign_in: SignIn::SignedOut,
  };
  assert_eq!(GitHubState::of(Some(&signed_out), None), GitHubState::NotSignedIn);
  for (result, state) in [
    (GitHubCheckResult::ExecutableMissing, GitHubState::ProgramMissing),
    (GitHubCheckResult::NotRecognized, GitHubState::Unrecognized),
    (GitHubCheckResult::CouldNotRun, GitHubState::CouldNotRun),
  ] {
    assert_eq!(GitHubState::of(Some(&result), None), state);
  }
}

#[test]
fn state_missing_scope_only_when_scopes_are_listed_without_repo() {
  let without_repo = signed_in("ivanzaida", &["read:org"]);
  assert_eq!(
    GitHubState::of(Some(&without_repo), None),
    GitHubState::MissingScope {
      account: login("ivanzaida")
    }
  );
  // Fine-grained and app tokens list no scopes; that says nothing about access.
  let unscoped = signed_in("ivanzaida", &[]);
  assert!(matches!(
    GitHubState::of(Some(&unscoped), None),
    GitHubState::SignedIn { .. }
  ));
}

#[test]
fn state_advice_commands_are_for_the_connection_host() {
  let host = GitHubHost::new("ghe.example.com").expect("valid host");
  assert_eq!(
    GhAdvice::Login.command(&host),
    "gh auth login --hostname ghe.example.com"
  );
  assert_eq!(
    GhAdvice::Switch(Some(login("ivanzaida"))).command(&host),
    "gh auth switch -h ghe.example.com -u ivanzaida"
  );
  assert_eq!(
    GhAdvice::Switch(None).command(&GitHubHost::github_com()),
    "gh auth switch -h github.com -u <login>"
  );
  assert_eq!(
    GhAdvice::Refresh.command(&GitHubHost::github_com()),
    "gh auth refresh -h github.com -s repo"
  );
  let wrong = GitHubState::WrongAccount {
    active: login("ivan-work"),
    expected: login("ivanzaida"),
  };
  assert_eq!(wrong.advice(), Some(GhAdvice::Switch(Some(login("ivanzaida")))));
  assert_eq!(GitHubState::ProgramMissing.advice(), None);
}

#[test]
fn state_expected_account_is_what_gh_uses_now() {
  let repo = GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur").expect("valid repository");
  let view = RepositoryView {
    repo,
    default_branch: BranchName::new("main").expect("valid branch"),
    permission: RepoPermission::Admin,
  };
  let mut link = RepositoryLink::new(
    ConnectionId::generate(),
    RemoteName::origin(),
    view,
    login("ivanzaida"),
    now(),
  );
  link.expect_account(Some(&login("ivan-work"))).expect("signed in");
  assert_eq!(link.expected_login(), &login("ivan-work"));
  assert_eq!(
    link.expect_account(None),
    Err(RepositoryError::NotSignedIn(GitHubHost::github_com()))
  );
  assert_eq!(link.expected_login(), &login("ivan-work"));
}
