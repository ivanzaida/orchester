//! The list: links to tasks, tabs, search, sort, footer, status and links.

use orchester_core::{
  connection::GitHubHost,
  pull_request::{PullRequest, PullRequestState},
  time::Timestamp,
};
use orchester_workspace::pull_requests::{
  Inbox,
  list::{Moment, PullRequestList, PullRequestSort, PullRequestTab, load_list_in},
  load_in,
  status::{Remedy, SyncStatus},
};

use super::{
  link, login, numbers, pull_request, seeded,
  support::{Fixture, at},
};

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
fn pull_requests_say_when_they_were_merged_or_closed_else_updated() {
  let (_fixture, list) = seeded();
  let moment = |number| list.find(number).expect("listed").moment();
  assert_eq!(moment(131), Moment::Merged(at(550)));
  assert_eq!(moment(127), Moment::Closed(at(450)));
  assert_eq!(moment(142), Moment::Updated(at(900)));
}

#[test]
fn pull_requests_of_a_fork_and_paused_runs_are_not_agents_at_work() {
  use orchester_core::store::PullRequestStore;
  let mut fixture = Fixture::keyed("KON");
  link(&mut fixture);
  let task = fixture.task("Add retry to webhook sender", at(10));
  let mut run = fixture.started_run(task, at(20));
  let events = run.request_pause(at(30)).expect("a pause is asked");
  fixture.record(&run, &events);
  let ours = pull_request(142, "orchester/orc-run-1", PullRequestState::Open, None, 900);
  let mut fork = super::parts(143, "orchester/orc-run-1", PullRequestState::Open, None, 800);
  fork.cross_repository = true;
  let fork = PullRequest::new(&super::repo(), fork).expect("a pull request");
  fixture
    .db
    .replace_pull_requests(&super::repo(), &[ours, fork], at(1_000))
    .expect("stored");
  let PullRequestList::Linked(list) = load_list_in(fixture.repo.path()).expect("the list") else {
    panic!("the project is linked");
  };
  let linked = list
    .find(142)
    .and_then(|listed| listed.task.clone())
    .expect("#142 is KON-1's run's");
  assert!(!linked.agent_working, "a pause was asked of the run");
  assert_eq!(
    list.find(143).and_then(|listed| listed.task.clone()),
    None,
    "a fork's branch is not the run's"
  );
  assert_eq!(list.count(PullRequestTab::AgentWorking), 0);
}
