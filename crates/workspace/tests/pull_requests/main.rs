//! A run's pull request: creating one from a task's run (`create`) with the sync that follows, and whether it
//! would carry the local base's unpushed commits (`unpushed`); a run's pull request's CI (`ci`) and Fix CI
//! (`ci_fix`), through a fake `gh`.

#[path = "../support/mod.rs"]
mod support;

mod ci;
mod ci_fix;
mod create;
mod unpushed;

use orchester_core::{
  connection::{GitHubCheck, GitHubCheckResult, GitHubHost, GitHubLogin, SignIn},
  diff::Changes,
  name::Name,
  pull_request::{CheckRollup, PullRequest, PullRequestParts, PullRequestState, ReviewDecision},
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryLink, RepositoryView},
  revision::Revision,
  run::BranchName,
  store::RepositoryStore,
};
use orchester_workspace::connections::github::{GitHubDraft, save_github};
use support::{Fixture, at};

pub fn repo() -> GitHubRepo {
  GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur").expect("valid repository")
}

pub fn login(value: &str) -> GitHubLogin {
  GitHubLogin::new(value).expect("valid login")
}

/// Pull request `number` from branch `head`, in `state`, updated at `updated`; merged or closed 50 ms before.
pub fn pull_request(
  number: u64,
  head: &str,
  state: PullRequestState,
  review: Option<ReviewDecision>,
  updated: i64,
) -> PullRequest {
  PullRequest::new(&repo(), parts(number, head, state, review, updated)).expect("a pull request")
}

pub fn parts(
  number: u64,
  head: &str,
  state: PullRequestState,
  review: Option<ReviewDecision>,
  updated: i64,
) -> PullRequestParts {
  let head_sha = Revision::new(&format!("{number:0>40}")).expect("revision");
  PullRequestParts {
    number,
    title: format!("Pull request {number}"),
    url: format!("https://github.com/ivanzaida/kontur/pull/{number}"),
    state,
    head: BranchName::new(head).expect("branch"),
    base: BranchName::new("main").expect("branch"),
    checks: CheckRollup::none(head_sha.clone()),
    head_sha,
    author: Some("ivanzaida".to_owned()),
    changes: Changes {
      files: 2,
      additions: 10,
      deletions: 1,
    },
    review_decision: review,
    updated_at: at(updated),
    merged_at: (state == PullRequestState::Merged).then(|| at(updated - 50)),
    closed_at: matches!(state, PullRequestState::Merged | PullRequestState::Closed).then(|| at(updated - 50)),
    cross_repository: false,
  }
}

/// Links the fixture's project to `ivanzaida/kontur` through a GitHub connection.
pub fn link(fixture: &mut Fixture) {
  let draft = GitHubDraft {
    id: None,
    name: Name::new("GitHub · github.com").expect("valid name"),
    host: GitHubHost::github_com(),
    executable: fixture.repo.path().join("gh.exe"),
    check: GitHubCheck {
      at: at(1),
      result: GitHubCheckResult::Available {
        version: "2.97.0".into(),
        account: Some(login("ivanzaida")),
        scopes: vec!["repo".into()],
        sign_in: SignIn::SignedIn,
      },
    },
  };
  let connection = save_github(&mut fixture.db, draft, at(1)).expect("the GitHub connection is saved");
  let view = RepositoryView {
    repo: repo(),
    default_branch: BranchName::new("main").expect("branch"),
    permission: RepoPermission::Write,
  };
  let link = RepositoryLink::new(connection, RemoteName::origin(), view, login("ivanzaida"), at(1));
  fixture.db.save_repository_link(&link).expect("the link is saved");
}
