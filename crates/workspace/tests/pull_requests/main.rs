//! The Pull requests page's list: the stored pull requests of the linked repository, open and closed, each
//! with the task whose run made its branch; the page's tabs, search, sort and footer; and the deprecated
//! desktop's inbox, which keeps only the open ones (`list`); syncing them through a fake `gh`: a rate limit
//! that holds every later sync, how the latest sync went, and the closed ones a sync of the open ones keeps
//! (`sync`); and creating one from a task's run (`create`).

#[path = "../support/mod.rs"]
mod support;

mod create;
mod feedback;
mod list;
mod sync;

use orchester_core::{
  connection::{GitHubCheck, GitHubCheckResult, GitHubHost, GitHubLogin, SignIn},
  diff::Changes,
  name::Name,
  pull_request::{CheckRollup, PullRequest, PullRequestParts, PullRequestState, ReviewDecision},
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryLink, RepositoryView},
  revision::Revision,
  run::BranchName,
  store::{PullRequestStore, RepositoryStore},
};
use orchester_workspace::{
  connections::github::{GitHubDraft, save_github},
  pull_requests::list::{LinkedList, PullRequestList, PullRequestSort, PullRequestTab, load_list_in},
};
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

/// KON-1 runs on `orchester/orc-run-1`, KON-2's run on `orchester/orc-run-2` succeeded; a sync stored an
/// open pull request of each, a draft and a merged one of no run, and a closed one.
pub fn seeded() -> (Fixture, LinkedList) {
  let mut fixture = Fixture::keyed("KON");
  let working = fixture.task("Add retry to webhook sender", at(10));
  fixture.started_run(working, at(20));
  let done = fixture.task("Typed errors per module", at(11));
  let mut run = fixture.started_run(done, at(21));
  fixture.succeed(&mut run, at(30));
  link(&mut fixture);
  let stored = [
    pull_request(
      142,
      "orchester/orc-run-1",
      PullRequestState::Open,
      Some(ReviewDecision::ReviewRequired),
      900,
    ),
    pull_request(
      137,
      "orchester/orc-run-2",
      PullRequestState::Open,
      Some(ReviewDecision::ChangesRequested),
      800,
    ),
    pull_request(139, "deps/tokio", PullRequestState::Draft, None, 700),
    pull_request(
      131,
      "readme-badges",
      PullRequestState::Merged,
      Some(ReviewDecision::Approved),
      600,
    ),
    pull_request(127, "spike", PullRequestState::Closed, None, 500),
  ];
  fixture
    .db
    .replace_pull_requests(&repo(), &stored, at(1_000))
    .expect("the pull requests are stored");
  let PullRequestList::Linked(list) = load_list_in(fixture.repo.path()).expect("the list") else {
    panic!("the project is linked");
  };
  (fixture, *list)
}

pub fn numbers(list: &LinkedList, tab: PullRequestTab, query: &str, sort: PullRequestSort) -> Vec<u64> {
  list
    .rows(tab, query, sort)
    .iter()
    .map(|listed| listed.pull_request.number())
    .collect()
}
