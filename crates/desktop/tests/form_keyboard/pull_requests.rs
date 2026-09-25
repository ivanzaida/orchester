//! Pull requests (08.01 `wGtxT` and its states 08.D–08.H), driven with a fake `gh` that answers
//! from captured and synthetic output (`crates/core/tests/fixtures/github`) and counts what ran.
//! The real `gh` never runs, and Open in GitHub is not pressed: it would open the browser of the
//! machine running the tests.

use std::{
  collections::HashMap,
  sync::Arc,
  time::{Duration, Instant},
};

use lurq::app::events::ScrollPhase;
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

pub(crate) const AUTH_STATUS: &str = "auth status --hostname github.com --json hosts";
pub(crate) const LIST: &str = "pr list -R github.com/ivanzaida/kontur --state open --limit 100 --json \
                    number,title,url,state,isDraft,headRefName,baseRefName,headRefOid,author,additions,deletions,\
                    changedFiles,updatedAt,reviewDecision,statusCheckRollup";
const RATE_LIMIT: &str = "api --hostname github.com rate_limit";
pub(crate) const OPEN_PULL_REQUESTS: &str =
  include_str!("../../../core/tests/fixtures/github/gh_pr_list_synthetic_open.stdout");
const NO_PULL_REQUESTS: &str = include_str!("../../../core/tests/fixtures/github/gh_pr_list_orchester_open.stdout");
pub(crate) const NETWORK: &str = include_str!("../../../core/tests/fixtures/github/gh_repo_view_network.stderr");
const RATE_LIMITED: &str = include_str!("../../../core/tests/fixtures/github/gh_repo_view_rate_limited.stderr");
/// Signed in as ivanzaida without the `repo` scope; shaped like the captured output.
const NO_REPO_SCOPE: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","tokenSource":"keyring","scopes":"read:org","gitProtocol":"https"}]}}"#;
const LIST_ID: &str = "pull-requests-list";
const STATUS: &str = "pull-requests-sync-status";

/// `gh` answering read-only commands from a table that tests change between syncs.
pub(crate) struct FakeGh {
  answers: Mutex<HashMap<&'static str, Result<CommandOutput, CommandError>>>,
  pub(crate) ran: Mutex<Vec<String>>,
}

impl FakeGh {
  pub(crate) fn signed_in(auth: &str, list: Result<CommandOutput, CommandError>) -> Arc<Self> {
    Arc::new(Self {
      answers: Mutex::new(HashMap::from([
        ("--version", Ok(ok(VERSION))),
        (AUTH_STATUS, Ok(ok(auth))),
        (LIST, list),
      ])),
      ran: Mutex::new(Vec::new()),
    })
  }

  pub(crate) fn answer(&self, args: &'static str, output: Result<CommandOutput, CommandError>) {
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

pub(crate) fn ok(stdout: &str) -> CommandOutput {
  CommandOutput {
    success: true,
    stdout: stdout.to_owned(),
    stderr: String::new(),
  }
}

pub(crate) fn failed(stderr: &str) -> CommandOutput {
  CommandOutput {
    success: false,
    stdout: String::new(),
    stderr: stderr.to_owned(),
  }
}

/// A project linked to `ivanzaida/kontur` that expects `expected`, with `gh` answered by `gh`.
pub(crate) fn linked(gh: &Arc<FakeGh>, expected: &str) -> (Desktop, ConnectionId) {
  let desktop = Desktop::with_project_and_programs(gh.clone());
  let connection = github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(SIGNED_IN)));
  link(&desktop, connection, expected);
  (desktop, connection)
}

pub(crate) fn open_pull_requests(desktop: &mut Desktop) {
  desktop.wait_for("nav-pull-requests");
  desktop.click("nav-pull-requests");
  desktop.wait_for(STATUS);
}

pub(crate) fn settle_on(desktop: &mut Desktop, id: &'static str, text: &'static str) {
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

/// `count` open pull requests shaped like the synthetic ones, numbered from 1.
fn many_pull_requests(count: u64) -> String {
  let items: Vec<String> = (1..=count)
    .map(|number| {
      format!(
        r#"{{"additions":1,"author":{{"login":"ivanzaida"}},"baseRefName":"main","changedFiles":1,"deletions":0,"headRefName":"orc/change-{number}","headRefOid":"{number:0>40}","isDraft":false,"number":{number},"reviewDecision":"","state":"OPEN","statusCheckRollup":[],"title":"Change {number}","updatedAt":"2026-09-25T10:00:00Z","url":"https://github.com/ivanzaida/kontur/pull/{number}"}}"#
      )
    })
    .collect();
  format!("[{}]", items.join(","))
}

fn assert_near(found: f32, expected: f32) {
  assert!((found - expected).abs() < 0.5, "{found} is not {expected}");
}

/// Bounds of the first mounted row, or of the one titled `title`.
fn row_bounds(desktop: &mut Desktop, title: Option<&'static str>) -> lurq::core::ElementRect {
  desktop
    .tree
    .find_element(move |element| {
      element.id() == Some("pull-request-row") && title.is_none_or(|title| holds_text(element, title))
    })
    .unwrap_or_else(|| panic!("a row titled {title:?} is mounted"))
    .bounds()
}

fn holds_text(element: lurq::node::ElementRef<'_>, text: &str) -> bool {
  element.text_content() == Some(text) || element.children().iter().any(|child| holds_text(child, text))
}

fn bounds(desktop: &mut Desktop, id: &str) -> (f32, f32) {
  let bounds = desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (bounds.y, bounds.y + bounds.height)
}

#[test]
fn pull_requests_footer_follows_the_rows_or_the_scrolled_list() {
  let gh = FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS)));
  let (mut desktop, _) = linked(&gh, "ivanzaida");
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, LIST_ID, "3 open pull requests");
  // Headings, rows and footer are 20 apart; three rows fit, so the footer follows the last.
  let (_, headings_bottom) = bounds(&mut desktop, "pull-requests-headings");
  let (footer_top, _) = bounds(&mut desktop, "pull-requests-footer");
  assert_near(footer_top - headings_bottom, 20.0 + 300.0 + 20.0);
  let first = row_bounds(&mut desktop, None);
  assert_near(first.y - headings_bottom, 20.0);

  gh.answer(LIST, Ok(ok(&many_pull_requests(12))));
  desktop.click("pull-requests-refresh");
  settle_on(&mut desktop, LIST_ID, "12 open pull requests");
  let (_, list_bottom) = bounds(&mut desktop, LIST_ID);
  let (_, headings_bottom) = bounds(&mut desktop, "pull-requests-headings");
  let (footer_top, footer_bottom) = bounds(&mut desktop, "pull-requests-footer");
  // Twelve rows do not fit: the rows scroll and the footer stays inside the screen.
  assert!(footer_top - headings_bottom < 20.0 + 1200.0 + 20.0);
  assert!(footer_bottom <= list_bottom);
  // Newest first: scrolled to the end, the last row (#1) ends 20 above the footer, not cut by it.
  desktop.tree.scroll(
    700.0,
    (headings_bottom + footer_top) / 2.0,
    0.0,
    -5000.0,
    ScrollPhase::Scroll,
  );
  desktop.pass();
  let last = row_bounds(&mut desktop, Some("Change 1"));
  assert_near(footer_top - (last.y + last.height), 20.0);

  desktop.press("pull-requests-search");
  desktop.type_text("Change 1");
  // "Change 1", "Change 10", "Change 11", "Change 12".
  assert!(shows(
    &mut desktop,
    "pull-requests-footer",
    "4 of 12 open pull requests"
  ));
}
