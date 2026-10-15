//! The Pull requests page's list: the stored pull requests of the linked repository, open and closed, each
//! with the task whose run made its branch; the page's tabs, search, sort and footer; and the deprecated
//! desktop's inbox, which keeps only the open ones.

mod support;

use orchester_core::{
  connection::{GitHubCheck, GitHubCheckResult, GitHubHost, GitHubLogin, SignIn},
  diff::Changes,
  name::Name,
  pull_request::{CheckRollup, PullRequest, PullRequestParts, PullRequestState, ReviewDecision},
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryLink, RepositoryView},
  revision::Revision,
  run::BranchName,
  store::{PullRequestStore, RepositoryStore},
  time::Timestamp,
};
use orchester_workspace::{
  connections::github::{GitHubDraft, save_github},
  pull_requests::{
    Inbox, links,
    list::{LinkedList, PullRequestList, PullRequestSort, PullRequestTab, load_list_in},
    load_in,
    status::{Remedy, SyncStatus},
  },
};
use support::{Fixture, at};

fn repo() -> GitHubRepo {
  GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur").expect("valid repository")
}

fn login(value: &str) -> GitHubLogin {
  GitHubLogin::new(value).expect("valid login")
}

/// Pull request `number` from branch `head`, in `state`, updated at `updated`.
fn pull_request(
  number: u64,
  head: &str,
  state: PullRequestState,
  review: Option<ReviewDecision>,
  updated: i64,
) -> PullRequest {
  let head_sha = Revision::new(&format!("{number:0>40}")).expect("revision");
  PullRequest::new(
    &repo(),
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
    },
  )
  .expect("a pull request")
}

/// Links the fixture's project to `ivanzaida/kontur` through a GitHub connection.
fn link(fixture: &mut Fixture) {
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
fn seeded() -> (Fixture, LinkedList) {
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

fn numbers(list: &LinkedList, tab: PullRequestTab, query: &str, sort: PullRequestSort) -> Vec<u64> {
  list
    .rows(tab, query, sort)
    .iter()
    .map(|listed| listed.pull_request.number())
    .collect()
}

#[test]
fn pull_requests_link_the_task_whose_run_made_their_branch() {
  let (_fixture, list) = seeded();
  let task = |number| list.find(number).and_then(|listed| listed.task.clone());
  let working = task(142).expect("#142 is KON-1's run's");
  assert_eq!(working.key.to_string(), "KON-1");
  assert_eq!(&*working.title, "Add retry to webhook sender");
  assert!(working.agent_working, "KON-1's run is running");
  let done = task(137).expect("#137 is KON-2's run's");
  assert_eq!(done.key.to_string(), "KON-2");
  assert!(!done.agent_working, "KON-2's run succeeded");
  assert_eq!(task(139), None, "no run made deps/tokio");
}

#[test]
fn pull_requests_fall_into_the_pages_tabs() {
  let (_fixture, list) = seeded();
  let counts = PullRequestTab::ALL.map(|tab| list.count(tab));
  assert_eq!(
    counts,
    [3, 1, 1, 1, 1],
    "open, needs review, agent working, merged, closed"
  );
  let recent = PullRequestSort::RecentlyUpdated;
  assert_eq!(numbers(&list, PullRequestTab::Open, "", recent), [142, 137, 139]);
  assert_eq!(numbers(&list, PullRequestTab::NeedsReview, "", recent), [142]);
  assert_eq!(numbers(&list, PullRequestTab::AgentWorking, "", recent), [142]);
  assert_eq!(numbers(&list, PullRequestTab::Merged, "", recent), [131]);
  assert_eq!(numbers(&list, PullRequestTab::Closed, "", recent), [127]);
}

#[test]
fn pull_requests_are_searched_by_task_key_and_sorted() {
  let (_fixture, list) = seeded();
  let open = PullRequestTab::Open;
  assert_eq!(numbers(&list, open, "kon-2", PullRequestSort::RecentlyUpdated), [137]);
  assert_eq!(numbers(&list, open, "#139", PullRequestSort::RecentlyUpdated), [139]);
  assert_eq!(numbers(&list, open, "deps/", PullRequestSort::RecentlyUpdated), [139]);
  assert_eq!(
    numbers(&list, open, "", PullRequestSort::LeastRecentlyUpdated),
    [139, 137, 142]
  );
  assert_eq!(numbers(&list, open, "", PullRequestSort::Number), [142, 139, 137]);
}

#[test]
fn pull_requests_footer_counts_a_tab_among_its_whole() {
  let (_fixture, list) = seeded();
  let footer = |tab, shown, query| {
    let message = list.footer(tab, shown, query);
    let arg = |name| {
      message
        .args
        .iter()
        .find(|(arg, _)| *arg == name)
        .map(|(_, value)| value.clone())
    };
    (message.key, arg("shown"), arg("count"))
  };
  let some = |value: &str| Some(value.to_owned());
  assert_eq!(
    footer(PullRequestTab::Open, 3, ""),
    ("pullRequests.footer.open.other", some("3"), some("3"))
  );
  assert_eq!(
    footer(PullRequestTab::NeedsReview, 1, ""),
    ("pullRequests.footer.open.part", some("1"), some("3"))
  );
  assert_eq!(
    footer(PullRequestTab::Open, 1, "kon"),
    ("pullRequests.footer.open.part", some("1"), some("3"))
  );
  assert_eq!(
    footer(PullRequestTab::Merged, 1, ""),
    ("pullRequests.footer.merged.one", some("1"), some("1"))
  );
}

#[test]
fn pull_requests_status_reports_the_last_sync() {
  let (_fixture, list) = seeded();
  assert_eq!(list.status(), SyncStatus::Synced(at(1_000)));
  assert!(!list.is_paused(Timestamp::now()));
}

#[test]
fn pull_requests_without_a_link_are_unavailable() {
  let fixture = Fixture::keyed("KON");
  assert_eq!(
    load_list_in(fixture.repo.path()).expect("the list"),
    PullRequestList::Unlinked
  );
}

#[test]
fn pull_requests_inbox_of_the_deprecated_desktop_keeps_the_open_ones() {
  let (fixture, _) = seeded();
  let Inbox::Linked(inbox) = load_in(fixture.repo.path()).expect("the inbox") else {
    panic!("the project is linked");
  };
  let synced = inbox.synced.expect("a sync stored the list");
  let numbers: Vec<u64> = synced.pull_requests.iter().map(PullRequest::number).collect();
  assert_eq!(numbers, [142, 137, 139]);
}

#[test]
fn pull_requests_failed_syncs_offer_what_fixes_them() {
  use orchester_core::{connection::GitHubState, pull_request::SyncFailure, repository::RepositoryError};
  let wrong = SyncFailure::Account(GitHubState::WrongAccount {
    active: login("sam-work"),
    expected: login("ivanzaida"),
  });
  assert_eq!(
    Remedy::of(&wrong),
    Remedy::CheckAgainOrUseAccount {
      active: login("sam-work")
    }
  );
  assert_eq!(
    SyncStatus::after(Some(at(5)), Some(&wrong)),
    SyncStatus::Paused {
      active: login("sam-work")
    }
  );
  let missing = SyncFailure::Account(GitHubState::ProgramMissing);
  assert_eq!(Remedy::of(&missing), Remedy::CheckAgainOrConnections);
  let limited = SyncFailure::RateLimited {
    resets_at: Some(at(10_000)),
  };
  assert_eq!(Remedy::of(&limited), Remedy::None);
  assert!(limited.is_paused(at(9_000)));
  let unreachable = SyncFailure::Repository(RepositoryError::Network {
    host: GitHubHost::github_com(),
    detail: "timeout".to_owned(),
  });
  assert_eq!(
    SyncStatus::after(Some(at(5)), Some(&unreachable)),
    SyncStatus::Unreachable { last: Some(at(5)) }
  );
}

#[test]
fn pull_requests_links_name_the_pages_routes() {
  let (fixture, list) = seeded();
  let project = orchester_store::ProjectDb::open(fixture.repo.path())
    .expect("project database")
    .project()
    .id();
  assert_eq!(
    links::pull_request(project, 142),
    format!("/project/{project}/pull-requests/142")
  );
  assert_eq!(
    links::pull_requests(project, PullRequestTab::Merged),
    format!("/project/{project}/pull-requests?tab=merged")
  );
  let task = list.find(142).and_then(|listed| listed.task.clone()).expect("a task");
  assert_eq!(
    links::task(project, &task.key),
    format!("/project/{project}/tasks/KON-1/summary")
  );
}
