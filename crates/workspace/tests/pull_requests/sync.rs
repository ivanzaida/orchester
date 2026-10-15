//! Syncing through a fake `gh` that answers as `gh` 2.97 does: how the latest sync went is kept and reported,
//! a rate limit holds every later sync until it resets, a client's refresh waits 30 s after a sync, and a
//! sync of the open ones keeps the closed and merged ones a full sync stored.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::GitHubState,
  pull_request::{PullRequestState, SyncFailure},
  store::PullRequestStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::pull_requests::{
  list::{LinkedList, PullRequestList, PullRequestTab, load_list_in, refresh_list_in, sync_list_in},
  status::SyncStatus,
  sync_in,
};
use parking_lot::Mutex;

use super::{link, pull_request, repo, support::Fixture};

/// How the fake answers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
  Ok,
  SignedOut,
  RateLimited,
}

/// `gh` answering a sync: the open list it holds, no closed one, no checks.
struct FakeGh {
  mode: Mutex<Mode>,
  open: String,
  /// The `pr list` requests it answered.
  lists: AtomicUsize,
}

impl FakeGh {
  fn new(mode: Mode, open: &[(u64, &str)]) -> Arc<Self> {
    let listed: Vec<String> = open
      .iter()
      .map(|(number, head)| {
        format!(
          r#"{{"number":{number},"title":"Pull request {number}","url":"https://github.com/ivanzaida/kontur/pull/{number}","state":"OPEN","isDraft":false,"headRefName":"{head}","baseRefName":"main","headRefOid":"{number:0>40}","author":{{"login":"ivanzaida"}},"additions":1,"deletions":0,"changedFiles":1,"updatedAt":"2026-10-01T10:00:00Z","reviewDecision":"","mergedAt":null,"closedAt":null,"isCrossRepository":false}}"#
        )
      })
      .collect();
    Arc::new(Self {
      mode: Mutex::new(mode),
      open: format!("[{}]", listed.join(",")),
      lists: AtomicUsize::new(0),
    })
  }

  fn lists(&self) -> usize {
    self.lists.load(Ordering::SeqCst)
  }
}

fn answer(code: i32, stdout: &str, stderr: &str) -> Result<CommandOutput, CommandError> {
  Ok(CommandOutput {
    code: Some(code),
    stdout: stdout.to_owned(),
    stderr: stderr.to_owned(),
  })
}

impl CommandRunner for FakeGh {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let args = command.args().join(" ");
    let mode = *self.mode.lock();
    if args == "--version" {
      return answer(0, "gh version 2.97.0 (2026-07-31)\n", "");
    }
    if args.starts_with("auth status") {
      return match mode {
        Mode::SignedOut => answer(1, "{\"hosts\":{}}", "You are not logged into any GitHub hosts."),
        Mode::Ok | Mode::RateLimited => answer(
          0,
          r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","scopes":"repo"}]}}"#,
          "",
        ),
      };
    }
    if args.starts_with("pr list") {
      self.lists.fetch_add(1, Ordering::SeqCst);
      if mode == Mode::RateLimited {
        return answer(1, "", "GraphQL: API rate limit already exceeded for user ID 1.");
      }
      return answer(
        0,
        if args.contains("--state open") {
          &self.open
        } else {
          "[]"
        },
        "",
      );
    }
    if args.starts_with("api graphql") {
      let asked = command.stdin().unwrap_or_default().matches("object(oid:").count();
      let commits: Vec<String> = (0..asked).map(|index| format!(r#""c{index}":null"#)).collect();
      return answer(
        0,
        &format!(r#"{{"data":{{"repository":{{{}}}}}}}"#, commits.join(",")),
        "",
      );
    }
    if args.ends_with("rate_limit") {
      // Resets in an hour.
      let reset = Timestamp::now().unix_millis() / 1000 + 3600;
      return answer(0, &format!(r#"{{"resources":{{"graphql":{{"reset":{reset}}}}}}}"#), "");
    }
    panic!("`{args}` is not a command of a sync");
  }
}

/// A linked project whose `gh` is `gh`.
fn linked(gh: &Arc<FakeGh>) -> (Fixture, Programs) {
  let mut fixture = Fixture::keyed("KON");
  link(&mut fixture);
  (fixture, Programs::with_runner(gh.clone()))
}

fn listed(list: PullRequestList) -> LinkedList {
  match list {
    PullRequestList::Linked(list) => *list,
    PullRequestList::Unlinked => panic!("the project is linked"),
  }
}

#[test]
fn pull_requests_sync_failure_is_kept_and_reported_when_read_again() {
  let gh = FakeGh::new(Mode::SignedOut, &[(142, "work")]);
  let (fixture, programs) = linked(&gh);
  let synced = listed(sync_list_in(fixture.repo.path(), &programs).expect("a sync"));
  assert_eq!(
    synced.failure,
    Some(SyncFailure::Account(GitHubState::NotSignedIn)),
    "gh signed out of every host is not signed in"
  );
  let read = listed(load_list_in(fixture.repo.path()).expect("the list"));
  assert_eq!(read.status(), SyncStatus::Account(GitHubState::NotSignedIn));
  assert!(read.attempted_at.is_some());
  *gh.mode.lock() = Mode::Ok;
  let synced = listed(sync_list_in(fixture.repo.path(), &programs).expect("a sync"));
  assert_eq!(synced.failure, None);
  let read = listed(load_list_in(fixture.repo.path()).expect("the list"));
  assert!(matches!(read.status(), SyncStatus::Synced(_)));
}

#[test]
fn pull_requests_rate_limit_holds_every_sync_until_it_resets() {
  let gh = FakeGh::new(Mode::RateLimited, &[(142, "work")]);
  let (fixture, programs) = linked(&gh);
  let synced = listed(sync_list_in(fixture.repo.path(), &programs).expect("a sync"));
  assert!(synced.is_paused(Timestamp::now()), "{:?}", synced.failure);
  assert_eq!(gh.lists(), 1);
  *gh.mode.lock() = Mode::Ok;
  // A page opened again, or a client's refresh, asks GitHub nothing until the reset.
  let again = listed(sync_list_in(fixture.repo.path(), &programs).expect("a sync"));
  let later = Timestamp::from_unix_millis(Timestamp::now().unix_millis() + 60_000);
  let refreshed = listed(refresh_list_in(fixture.repo.path(), &programs, later).expect("a refresh"));
  assert_eq!(gh.lists(), 1, "nothing asked GitHub during the rate limit");
  assert!(matches!(again.status(), SyncStatus::RateLimited { resets_at: Some(_) }));
  assert!(matches!(refreshed.status(), SyncStatus::RateLimited { .. }));
}

#[test]
fn pull_requests_refresh_answers_from_storage_within_30_seconds_of_a_sync() {
  let gh = FakeGh::new(Mode::Ok, &[(142, "work")]);
  let (fixture, programs) = linked(&gh);
  let now = Timestamp::now();
  listed(refresh_list_in(fixture.repo.path(), &programs, now).expect("a refresh"));
  let lists = gh.lists();
  let soon = Timestamp::from_unix_millis(now.unix_millis() + 10_000);
  listed(refresh_list_in(fixture.repo.path(), &programs, soon).expect("a refresh"));
  assert_eq!(gh.lists(), lists, "a sync 10 s ago answers");
  let later = Timestamp::from_unix_millis(now.unix_millis() + 31_000);
  listed(refresh_list_in(fixture.repo.path(), &programs, later).expect("a refresh"));
  assert!(gh.lists() > lists, "31 s later the refresh syncs");
}

#[test]
fn pull_requests_sync_of_the_open_ones_keeps_the_stored_closed_ones() {
  let gh = FakeGh::new(Mode::Ok, &[(142, "work")]);
  let (mut fixture, programs) = linked(&gh);
  let stored = [
    pull_request(142, "work", PullRequestState::Open, None, 900),
    pull_request(140, "gone", PullRequestState::Open, None, 850),
    pull_request(131, "readme", PullRequestState::Merged, None, 800),
    pull_request(127, "spike", PullRequestState::Closed, None, 700),
  ];
  fixture
    .db
    .replace_pull_requests(&repo(), &stored, Timestamp::from_unix_millis(1_000))
    .expect("stored");
  // The deprecated desktop's sync reads the open ones only.
  sync_in(fixture.repo.path(), &programs).expect("a sync");
  let read = listed(load_list_in(fixture.repo.path()).expect("the list"));
  let numbers: Vec<u64> = read
    .pull_requests
    .iter()
    .map(|listed| listed.pull_request.number())
    .collect();
  assert!(numbers.contains(&131) && numbers.contains(&127), "{numbers:?}");
  assert!(
    !numbers.contains(&140),
    "an open one the sync did not read is gone: {numbers:?}"
  );
  assert_eq!(
    (read.count(PullRequestTab::Merged), read.count(PullRequestTab::Closed)),
    (1, 1)
  );
}
