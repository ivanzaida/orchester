//! Pull requests (08.01 `wGtxT` and its states 08.D–08.H), driven with a fake `gh` that answers
//! from captured and synthetic output (`crates/core/tests/fixtures/github`) and counts what ran.
//! The real `gh` never runs, and Open in GitHub is not pressed: it would open the browser of the
//! machine running the tests.

use std::{
  collections::HashMap,
  sync::Arc,
  time::{Duration, Instant},
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  id::ConnectionId,
  time::Timestamp,
};
use parking_lot::Mutex;

use crate::{
  github::{ReplayGh, SIGNED_IN, VERSION, github_connection, link, shows},
  harness::Desktop,
};

const AUTH_STATUS: &str = "auth status --hostname github.com --json hosts";
const LIST: &str = "pr list -R github.com/ivanzaida/kontur --state open --limit 100 --json \
                    number,title,url,state,isDraft,headRefName,baseRefName,headRefOid,author,additions,deletions,\
                    changedFiles,updatedAt,reviewDecision,statusCheckRollup";
const RATE_LIMIT: &str = "api --hostname github.com rate_limit";
const OPEN_PULL_REQUESTS: &str = include_str!("../../../core/tests/fixtures/github/gh_pr_list_synthetic_open.stdout");
const NO_PULL_REQUESTS: &str = include_str!("../../../core/tests/fixtures/github/gh_pr_list_orchester_open.stdout");
const NETWORK: &str = include_str!("../../../core/tests/fixtures/github/gh_repo_view_network.stderr");
const RATE_LIMITED: &str = include_str!("../../../core/tests/fixtures/github/gh_repo_view_rate_limited.stderr");
/// Signed in as ivanzaida without the `repo` scope; shaped like the captured output.
const NO_REPO_SCOPE: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","tokenSource":"keyring","scopes":"read:org","gitProtocol":"https"}]}}"#;
const LIST_ID: &str = "pull-requests-list";
const STATUS: &str = "pull-requests-sync-status";

/// `gh` answering read-only commands from a table that tests change between syncs.
struct FakeGh {
  answers: Mutex<HashMap<&'static str, Result<CommandOutput, CommandError>>>,
  ran: Mutex<Vec<String>>,
}

impl FakeGh {
  fn signed_in(auth: &str, list: Result<CommandOutput, CommandError>) -> Arc<Self> {
    Arc::new(Self {
      answers: Mutex::new(HashMap::from([
        ("--version", Ok(ok(VERSION))),
        (AUTH_STATUS, Ok(ok(auth))),
        (LIST, list),
      ])),
      ran: Mutex::new(Vec::new()),
    })
  }

  fn answer(&self, args: &'static str, output: Result<CommandOutput, CommandError>) {
    self.answers.lock().insert(args, output);
  }

  fn lists(&self) -> usize {
    self.ran.lock().iter().filter(|args| args.as_str() == LIST).count()
  }
}

impl CommandRunner for FakeGh {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let key = command.args().join(" ");
    self.ran.lock().push(key.clone());
    self
      .answers
      .lock()
      .get(key.as_str())
      .cloned()
      .unwrap_or_else(|| panic!("`{key}` is not a read-only gh command of the sync"))
  }
}

fn ok(stdout: &str) -> CommandOutput {
  CommandOutput {
    success: true,
    stdout: stdout.to_owned(),
    stderr: String::new(),
  }
}

fn failed(stderr: &str) -> CommandOutput {
  CommandOutput {
    success: false,
    stdout: String::new(),
    stderr: stderr.to_owned(),
  }
}

/// A project linked to `ivanzaida/kontur` that expects `expected`, with `gh` answered by `gh`.
fn linked(gh: &Arc<FakeGh>, expected: &str) -> (Desktop, ConnectionId) {
  let desktop = Desktop::with_project_and_programs(gh.clone());
  let connection = github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(SIGNED_IN)));
  link(&desktop, connection, expected);
  (desktop, connection)
}

fn open_pull_requests(desktop: &mut Desktop) {
  desktop.wait_for("nav-pull-requests");
  desktop.click("nav-pull-requests");
  desktop.wait_for(STATUS);
}

fn settle_on(desktop: &mut Desktop, id: &'static str, text: &'static str) {
  desktop.settle(text, |tree| {
    tree
      .get_element_by_id(id)
      .is_some_and(|element| crate::github::contains_text(element, text))
  });
}

#[test]
fn pull_requests_list_open_pull_requests_with_state_checks_and_no_linked_task() {
  let gh = FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS)));
  let (mut desktop, _) = linked(&gh, "ivanzaida");
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, STATUS, "Synced just now");
  for text in [
    "Add GitHub authentication",
    "#42 · No linked task · orc/github-auth → main",
    "6 files · +248 / −32 · Updated",
    "Open",
    "Review required",
    "3 checks passed",
    "on 8f3a2c1",
    "Add repository switcher",
    "Draft",
    "1 of 4 checks failed",
    "Update contributor documentation",
    "Changes requested",
    "No checks",
    "3 open pull requests",
    "ivanzaida / kontur",
  ] {
    assert!(shows(&mut desktop, LIST_ID, text), "{text}");
  }
  assert!(shows(&mut desktop, "pull-requests-tab-open", "3"));
  assert_eq!(gh.lists(), 1);
}

#[test]
fn pull_requests_search_filters_by_number_title_and_branch() {
  let gh = FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS)));
  let (mut desktop, _) = linked(&gh, "ivanzaida");
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, LIST_ID, "Add GitHub authentication");
  desktop.press("pull-requests-search");
  assert_eq!(desktop.focused().as_deref(), Some("pull-requests-search"));
  desktop.type_text("#43");
  assert!(shows(&mut desktop, LIST_ID, "Add repository switcher"));
  assert!(!shows(&mut desktop, LIST_ID, "Add GitHub authentication"));
  for _ in 0.."#43".len() {
    desktop.key("Backspace", "Backspace");
  }
  desktop.type_text("docs/");
  assert!(shows(&mut desktop, LIST_ID, "Update contributor documentation"));
  assert!(!shows(&mut desktop, LIST_ID, "Add repository switcher"));
  desktop.type_text("zzz");
  assert!(shows(&mut desktop, LIST_ID, "No open pull request matches “docs/zzz”."));
  // The search field is the screen's only Tab stop.
  desktop.tab();
  assert_eq!(desktop.focused().as_deref(), Some("pull-requests-search"));
}

#[test]
fn pull_requests_without_a_linked_repository_are_unavailable() {
  let gh = FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS)));
  let mut desktop = Desktop::with_project_and_programs(gh.clone());
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, STATUS, "GitHub not connected");
  assert!(shows(
    &mut desktop,
    "pull-requests-unavailable",
    "Pull requests are unavailable"
  ));
  desktop.click("pull-requests-refresh");
  assert!(gh.ran.lock().is_empty(), "nothing runs without a linked repository");
  desktop.click("pull-requests-open-connections");
  desktop.wait_for("add-connection");
}

#[test]
fn pull_requests_keep_the_list_when_github_is_unreachable() {
  let gh = FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS)));
  let (mut desktop, _) = linked(&gh, "ivanzaida");
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, STATUS, "Synced just now");
  gh.answer(LIST, Ok(failed(NETWORK)));
  desktop.click("pull-requests-refresh");
  settle_on(&mut desktop, STATUS, "Last synced just now · GitHub unreachable");
  assert!(shows(
    &mut desktop,
    "pull-requests-notice",
    "Showing pull requests from"
  ));
  assert!(shows(&mut desktop, "pull-requests-notice", "Couldn’t reach GitHub"));
  assert!(shows(&mut desktop, LIST_ID, "Add GitHub authentication"));
}

#[test]
fn pull_requests_rate_limit_shows_the_reset_and_pauses_sync() {
  let gh = FakeGh::signed_in(SIGNED_IN, Ok(failed(RATE_LIMITED)));
  // Synthetic `gh api rate_limit` answer: the GraphQL limit resets in 12 minutes.
  let reset = Timestamp::now().unix_millis() / 1000 + 12 * 60 - 1;
  let limits = format!(r#"{{"resources":{{"graphql":{{"limit":5000,"used":5000,"remaining":0,"reset":{reset}}}}}}}"#);
  gh.answer(RATE_LIMIT, Ok(ok(&limits)));
  let (mut desktop, _) = linked(&gh, "ivanzaida");
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, STATUS, "Rate limited · resets in 12 min");
  assert!(shows(&mut desktop, "pull-requests-notice", "GitHub rate limit reached"));
  assert!(shows(
    &mut desktop,
    "pull-requests-notice",
    "no requests left for ivanzaida"
  ));
  assert_eq!(gh.lists(), 1);
  // Refresh is off and the minute timer waits for the reset.
  desktop.click("pull-requests-refresh");
  desktop.tree.tick_timers_at(Instant::now() + Duration::from_secs(61));
  for _ in 0..20 {
    desktop.tree.tick_futures();
    desktop.pass();
  }
  assert_eq!(gh.lists(), 1);
}

#[test]
fn pull_requests_pause_for_another_account_until_the_project_expects_it() {
  let gh = FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS)));
  let (mut desktop, _) = linked(&gh, "ivan-work");
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, STATUS, "Paused · gh signed in as ivanzaida");
  assert!(shows(
    &mut desktop,
    "pull-requests-notice",
    "Signed in as ivanzaida, expected ivan-work"
  ));
  assert!(shows(
    &mut desktop,
    "github-command-switch",
    "gh auth switch -h github.com -u ivan-work"
  ));
  // Nothing was read as the wrong account.
  assert_eq!(gh.lists(), 0);
  desktop.click("pull-requests-use-account");
  settle_on(&mut desktop, STATUS, "Synced just now");
  assert!(shows(&mut desktop, LIST_ID, "Add GitHub authentication"));
  assert_eq!(gh.lists(), 1);
}

#[test]
fn pull_requests_name_the_missing_scope_and_its_command() {
  let gh = FakeGh::signed_in(NO_REPO_SCOPE, Ok(ok(OPEN_PULL_REQUESTS)));
  let (mut desktop, _) = linked(&gh, "ivanzaida");
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, STATUS, "Missing scope: repo");
  assert!(shows(&mut desktop, "pull-requests-notice", "without the repo scope"));
  assert!(shows(
    &mut desktop,
    "github-command-refresh",
    "gh auth refresh -h github.com -s repo"
  ));
  assert_eq!(gh.lists(), 0);
}

#[test]
fn pull_requests_say_when_none_are_open() {
  let gh = FakeGh::signed_in(SIGNED_IN, Ok(ok(NO_PULL_REQUESTS)));
  let (mut desktop, _) = linked(&gh, "ivanzaida");
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, LIST_ID, "No open pull requests");
  assert!(shows(
    &mut desktop,
    LIST_ID,
    "ivanzaida / kontur has no open pull requests"
  ));
}

#[test]
fn pull_requests_sync_every_minute_only_while_shown() {
  let gh = FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS)));
  let (mut desktop, _) = linked(&gh, "ivanzaida");
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, STATUS, "Synced just now");
  assert_eq!(gh.lists(), 1);
  let opened = Instant::now();
  desktop.tree.tick_timers_at(opened + Duration::from_secs(61));
  desktop.settle("the minute sync", |_| gh.lists() == 2);
  settle_on(&mut desktop, STATUS, "Synced just now");
  desktop.click("nav-tasks");
  desktop.tree.tick_timers_at(opened + Duration::from_secs(185));
  for _ in 0..20 {
    desktop.tree.tick_futures();
    desktop.pass();
  }
  assert_eq!(gh.lists(), 2, "no sync once another screen is shown");
}
