//! Pull requests keep their layout while they load (U10): until the stored inbox is known the
//! screen shows its topbar and heading and nothing that would move or vanish once it is; the Open
//! tab keeps room for its count; the sync status changing its text leaves Refresh in place. The
//! sync's `gh` waits behind a gate, so each frame before and after the stored inbox and the sync
//! arrive can be measured.

use std::{sync::Arc, time::Duration};

use lurq::core::ElementRect;
use orchester_core::command::{Command, CommandError, CommandOutput, CommandRunner};
use parking_lot::{Condvar, Mutex};

use crate::{
  github::{ReplayGh, SIGNED_IN, VERSION, github_connection, link, shows},
  harness::Desktop,
  pull_requests::{
    FakeGh, LIST, LIST_ID, NETWORK, OPEN_PULL_REQUESTS, STATUS, failed, many_pull_requests, ok, open_pull_requests,
    settle_on,
  },
};

const TABS: [&str; 5] = [
  "pull-requests-tab-open",
  "pull-requests-tab-needs-review",
  "pull-requests-tab-agent-working",
  "pull-requests-tab-merged",
  "pull-requests-tab-closed",
];
const REFRESH: &str = "pull-requests-refresh";
const HEADINGS: &str = "pull-requests-headings";
/// The screen description under its title (`hkCVZ`), which ends the heading.
const DESCRIPTION: &str = "Review changes, ask agents about the code, and send work back for a fix.";
/// A held `gh` lets go after this long, so a failing test ends instead of waiting for it forever.
const HOLD_LIMIT: Duration = Duration::from_secs(120);

/// [`FakeGh`] whose commands wait until the test opens the gate.
struct GatedGh {
  gh: Arc<FakeGh>,
  open: Mutex<bool>,
  opened: Condvar,
}

impl GatedGh {
  fn new(gh: Arc<FakeGh>, open: bool) -> Arc<Self> {
    Arc::new(Self {
      gh,
      open: Mutex::new(open),
      opened: Condvar::new(),
    })
  }

  fn set(&self, open: bool) {
    *self.open.lock() = open;
    self.opened.notify_all();
  }
}

/// Opens the gate when the test ends, passed or failed: the runtime waits for held commands.
struct OpenAtEnd(Arc<GatedGh>);

impl Drop for OpenAtEnd {
  fn drop(&mut self) {
    self.0.set(true);
  }
}

impl CommandRunner for GatedGh {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let mut open = self.open.lock();
    // A timed-out wait runs the command anyway; the test's assertions then fail on their own.
    self.opened.wait_while_for(&mut open, |open| !*open, HOLD_LIMIT);
    drop(open);
    self.gh.run(command)
  }
}

/// A project linked to `ivanzaida/kontur` whose `gh` waits behind `gh`'s gate.
fn linked(gh: &Arc<GatedGh>) -> Desktop {
  let desktop = Desktop::with_project_and_programs(gh.clone());
  let connection = github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(SIGNED_IN)));
  link(&desktop, connection, "ivanzaida");
  desktop
}

fn place(desktop: &mut Desktop, id: &str) -> Option<ElementRect> {
  desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
}

fn heading(desktop: &mut Desktop) -> ElementRect {
  desktop
    .tree
    .find_element(|element| element.text_content() == Some(DESCRIPTION))
    .expect("the screen description is shown")
    .bounds()
}

fn tabs(desktop: &mut Desktop) -> Vec<Option<ElementRect>> {
  TABS.iter().map(|id| place(desktop, id)).collect()
}

fn assert_same(found: ElementRect, expected: ElementRect, what: &str) {
  let near = |a: f32, b: f32| (a - b).abs() < 0.5;
  assert!(
    near(found.x, expected.x)
      && near(found.y, expected.y)
      && near(found.width, expected.width)
      && near(found.height, expected.height),
    "{what} moved from {expected:?} to {found:?}"
  );
}

/// `id` is laid out where it was, at `expected`.
fn assert_stays(desktop: &mut Desktop, id: &str, expected: ElementRect, what: &str) {
  let found = place(desktop, id).unwrap_or_else(|| panic!("{id} is laid out"));
  assert_same(found, expected, what);
}

fn assert_tabs_same(found: &[Option<ElementRect>], expected: &[Option<ElementRect>]) {
  for ((found, expected), id) in found.iter().zip(expected).zip(TABS) {
    let (found, expected) = (found.expect("a tab is laid out"), expected.expect("a tab is laid out"));
    assert_same(found, expected, id);
  }
}

fn assert_nothing_below_the_heading(desktop: &mut Desktop) {
  for id in TABS.iter().copied().chain([LIST_ID, "pull-requests-unavailable"]) {
    assert!(place(desktop, id).is_none(), "{id} is shown before the stored inbox");
  }
}

#[test]
fn pull_requests_layout_holds_while_the_stored_list_and_a_sync_arrive() {
  let gh = GatedGh::new(FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS))), true);
  let mut desktop = linked(&gh);
  let _open_at_end = OpenAtEnd(gh.clone());
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, STATUS, "Synced just now");
  let (heading_at, tabs_at) = (heading(&mut desktop), tabs(&mut desktop));
  let list_at = place(&mut desktop, LIST_ID).expect("the list is laid out");
  let headings_at = place(&mut desktop, HEADINGS).expect("the column headings are laid out");
  let refresh_at = place(&mut desktop, REFRESH).expect("Refresh is laid out");

  // Shown again, the screen reads its stored list while the sync waits behind the gate.
  desktop.click("nav-tasks");
  gh.set(false);
  desktop.click("nav-pull-requests");
  assert_same(heading(&mut desktop), heading_at, "the heading");
  assert_nothing_below_the_heading(&mut desktop);
  assert_stays(&mut desktop, REFRESH, refresh_at, "Refresh");

  desktop.settle("the stored list", |tree| tree.get_element_by_id(HEADINGS).is_some());
  assert!(shows(&mut desktop, STATUS, "Syncing"));
  assert_same(heading(&mut desktop), heading_at, "the heading");
  assert_tabs_same(&tabs(&mut desktop), &tabs_at);
  assert_stays(&mut desktop, LIST_ID, list_at, "the list");
  assert_stays(&mut desktop, HEADINGS, headings_at, "headings");
  assert_stays(&mut desktop, REFRESH, refresh_at, "Refresh while syncing");

  gh.set(true);
  settle_on(&mut desktop, STATUS, "Synced just now");
  assert_tabs_same(&tabs(&mut desktop), &tabs_at);
  assert_stays(&mut desktop, LIST_ID, list_at, "the list");
  assert_stays(&mut desktop, REFRESH, refresh_at, "Refresh after the sync");
}

#[test]
fn pull_requests_layout_keeps_room_for_the_first_count() {
  let gh = GatedGh::new(FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS))), false);
  let mut desktop = linked(&gh);
  let _open_at_end = OpenAtEnd(gh.clone());
  desktop.wait_for("nav-pull-requests");
  desktop.click("nav-pull-requests");
  let heading_at = heading(&mut desktop);
  assert_nothing_below_the_heading(&mut desktop);
  let refresh_at = place(&mut desktop, REFRESH).expect("Refresh is laid out");

  // Linked but never synced: the tab row and the list show at once, the Open tab without a count.
  desktop.settle("the stored inbox", |tree| tree.get_element_by_id(TABS[0]).is_some());
  assert!(!shows(&mut desktop, TABS[0], "3"));
  let (tabs_at, list_at) = (tabs(&mut desktop), place(&mut desktop, LIST_ID).expect("the list"));
  assert_same(heading(&mut desktop), heading_at, "the heading");

  gh.set(true);
  settle_on(&mut desktop, LIST_ID, "3 open pull requests");
  settle_on(&mut desktop, STATUS, "Synced just now");
  assert!(shows(&mut desktop, TABS[0], "3"));
  assert_tabs_same(&tabs(&mut desktop), &tabs_at);
  assert_stays(&mut desktop, LIST_ID, list_at, "the list");
  assert_same(heading(&mut desktop), heading_at, "the heading");
  assert_stays(&mut desktop, REFRESH, refresh_at, "Refresh");

  // A count with as many digits as a sync can read stays within the room kept for it.
  gh.gh.answer(LIST, Ok(ok(&many_pull_requests(12))));
  desktop.click(REFRESH);
  settle_on(&mut desktop, LIST_ID, "12 open pull requests");
  assert!(shows(&mut desktop, TABS[0], "12"));
  assert_tabs_same(&tabs(&mut desktop), &tabs_at);
}

#[test]
fn pull_requests_layout_without_a_link_never_shows_the_tab_row() {
  let mut desktop = Desktop::with_project_and_programs(FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS))));
  desktop.wait_for("nav-pull-requests");
  desktop.click("nav-pull-requests");
  let heading_at = heading(&mut desktop);
  assert_nothing_below_the_heading(&mut desktop);
  settle_on(&mut desktop, STATUS, "GitHub not connected");
  desktop.wait_for("pull-requests-unavailable");
  for id in TABS {
    assert!(place(&mut desktop, id).is_none(), "{id} is shown without a link");
  }
  assert_same(heading(&mut desktop), heading_at, "the heading");
}

#[test]
fn pull_requests_layout_keeps_refresh_in_place_in_the_narrowest_window() {
  let gh = GatedGh::new(FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS))), true);
  let mut desktop = linked(&gh);
  let _open_at_end = OpenAtEnd(gh.clone());
  // The window's minimum size (`main`).
  desktop.tree.resize(960, 700);
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, STATUS, "Synced just now");
  let refresh_at = place(&mut desktop, REFRESH).expect("Refresh is laid out");

  gh.set(false);
  gh.gh.answer(LIST, Ok(failed(NETWORK)));
  desktop.click(REFRESH);
  settle_on(&mut desktop, STATUS, "Syncing");
  assert_stays(&mut desktop, REFRESH, refresh_at, "Refresh while syncing");
  gh.set(true);
  settle_on(&mut desktop, STATUS, "Last synced just now · GitHub unreachable");
  assert_stays(&mut desktop, REFRESH, refresh_at, "Refresh after a failed sync");
}

/// The column headings, in order (`Qcgjy`).
const HEADING_TEXTS: [&str; 3] = ["PULL REQUEST / LINKED TASK", "STATE & REVIEW", "GITHUB CHECKS"];
/// What the first row (#42) shows under the State & review and the Checks headings.
const COLUMN_TEXTS: [&str; 2] = ["Review required", "3 checks passed"];
/// The least a row's title and meta keep, as the Tasks list keeps its titles' room (02.03E).
const TITLE_MIN: f32 = 200.0;

fn text_bounds(desktop: &mut Desktop, text: &str) -> ElementRect {
  desktop
    .tree
    .find_element(|element| element.text_content() == Some(text))
    .unwrap_or_else(|| panic!("{text} is shown"))
    .bounds()
}

#[test]
fn pull_requests_layout_columns_fit_the_narrowest_and_the_design_window() {
  for (width, open_in_rows) in [(960, false), (1440, true)] {
    let gh = GatedGh::new(FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS))), true);
    let mut desktop = linked(&gh);
    let _open_at_end = OpenAtEnd(gh.clone());
    desktop.tree.resize(width, 960);
    open_pull_requests(&mut desktop);
    settle_on(&mut desktop, LIST_ID, "3 open pull requests");

    let headings: Vec<ElementRect> = HEADING_TEXTS
      .iter()
      .map(|text| text_bounds(&mut desktop, text))
      .collect();
    for (pair, texts) in headings.windows(2).zip(HEADING_TEXTS.windows(2)) {
      assert!(
        pair[0].x + pair[0].width <= pair[1].x,
        "at {width}, {} runs into {}: {:?} and {:?}",
        texts[0],
        texts[1],
        pair[0],
        pair[1]
      );
    }
    // Each heading starts its column.
    for (heading, text) in headings[1..].iter().zip(COLUMN_TEXTS) {
      let column = text_bounds(&mut desktop, text);
      assert!(
        (heading.x - column.x).abs() < 0.5,
        "at {width}, {text} is under its heading"
      );
    }
    let identity = desktop.bounds("pull-request-identity");
    assert!(
      identity.width >= TITLE_MIN,
      "at {width}, a title keeps {} of {TITLE_MIN}",
      identity.width
    );
    assert_eq!(
      desktop.tree.get_element_by_id("pull-request-open").is_some(),
      open_in_rows,
      "at {width}, Open in GitHub in the rows"
    );
  }
}
