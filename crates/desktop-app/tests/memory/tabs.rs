//! The project's Memory page's tabs (`Memory with tabs` `wMr98`): their order with and without waiting proposals
//! (`HFISx` in `C0OJpC`), Approve and Reject in *Waiting for approval* moving a record to its tab, a search with no
//! match in the open tab naming the tab that holds one and opening it with the search kept (`vUhJo`), and nothing
//! past the window's right edge at 1440 and at 960 wide (`CIThk`, `ghfRl`). Only *Active* groups its records by type;
//! *Rejected* lists them without headings (`hxQhj`).

use orchester_app::routes::{MemoryTab, ProjectPage, Route};
use orchester_core::memory::MemoryStatus;

use super::harness::Page;

const ALL_STATUSES_WAITING_FIRST: [&str; 6] = ["waiting", "active", "deprecated", "superseded", "rejected", "settings"];

#[test]
fn memory_tabs_lead_with_waiting_while_proposals_wait() {
  let mut page = Page::seeded("memory");
  page.project_memory();
  let (order, shown) = page.tabs();
  assert_eq!(order, ALL_STATUSES_WAITING_FIRST);
  assert_eq!(shown, "waiting", "the page opens on what waits");
  assert_eq!(page.text("memory-tab-waiting"), "Waiting for approval 4");
  assert_eq!(page.text("memory-tab-waiting-count"), "4");
  assert_eq!(page.text("memory-tab-active"), "Active 10");
  assert_eq!(page.text("memory-tab-rejected"), "Rejected 1");
  assert_eq!(page.text("memory-tab-settings"), "Settings");
  // The Agents hub's Memory tab counts what waits for the person (05).
  page.until("the hub's counts", |window| {
    window.texts("agents-hub-tab-memory-count").is_some()
  });
  assert_eq!(page.text("agents-hub-tab-memory-count"), "4");
}

#[test]
fn memory_tabs_follow_active_while_nothing_waits() {
  let mut page = Page::seeded("tasks");
  page.project_memory();
  let (order, shown) = page.tabs();
  assert_eq!(
    order,
    ["active", "waiting", "deprecated", "superseded", "rejected", "settings"]
  );
  assert_eq!(shown, "active", "the page opens on Active");
  assert!(!page.has("memory-tab-waiting-count"), "no count while nothing waits");
  assert_eq!(
    page.text("memory-tab-active"),
    "Active",
    "a tab whose count is 0 shows its name only"
  );
  let empty = page.text("memory-empty");
  assert!(empty.starts_with("No active records"), "{empty}");
  assert!(!page.has("memory-search"), "an empty tab hides its search and filters");
  page.memory_tab(MemoryTab::Waiting);
  let empty = page.text("memory-empty");
  assert!(empty.starts_with("No records wait for approval"), "{empty}");
  assert!(
    empty.contains("Records agents propose for Kontur and its tasks"),
    "{empty}"
  );
}

#[test]
fn memory_waiting_tab_approves_in_place_and_the_record_moves_to_active() {
  let mut page = Page::seeded("memory");
  page.project_memory();
  let typed = page.record("API handlers return typed errors").id();
  let old = page.record("Handlers map domain errors").id();
  page.press_until(&format!("memory-proposal-{typed}-approve"), "the approval", |window| {
    window.text("memory-tab-waiting-count") == "3"
  });
  assert!(
    !page.has(&format!("memory-proposal-{typed}")),
    "it left the Waiting tab"
  );
  let (order, shown) = page.tabs();
  assert_eq!(
    order, ALL_STATUSES_WAITING_FIRST,
    "the order holds while the page shows"
  );
  assert_eq!(shown, "waiting", "the tab stays open where it is");
  assert_eq!(page.stored(typed).expect("kept").status(), MemoryStatus::Active);
  assert_eq!(page.stored(old).expect("kept").status(), MemoryStatus::Superseded);
  page.memory_tab(MemoryTab::Active);
  assert!(page.has(&format!("memory-record-{typed}")));
  page.memory_tab(MemoryTab::Superseded);
  assert!(page.has(&format!("memory-record-{old}")));
  assert_eq!(page.text("memory-tab-superseded"), "Superseded 2");
}

#[test]
fn memory_waiting_tab_rejects_in_place_and_the_record_moves_to_rejected() {
  let mut page = Page::seeded("memory");
  page.project_memory();
  let typed = page.record("API handlers return typed errors").id();
  page.press_until(&format!("memory-proposal-{typed}-reject"), "the rejection", |window| {
    window.text("memory-tab-waiting-count") == "3"
  });
  assert_eq!(page.stored(typed).expect("kept").status(), MemoryStatus::Rejected);
  assert_eq!(page.text("memory-tab-rejected"), "Rejected 2");
  page.memory_tab(MemoryTab::Rejected);
  let row = format!("memory-record-{typed}");
  assert!(page.has(&row));
  assert_eq!(page.text(&format!("{row}-status")), "Rejected");
}

#[test]
fn memory_rejected_tab_lists_without_type_headings_while_active_groups_by_type() {
  let mut page = Page::seeded("memory");
  let engine = page.record("Kontur keeps its data in SQLite");
  page.memory_tab(MemoryTab::Active);
  assert!(
    page.has(&format!("memory-group-{}", engine.content().kind().name())),
    "Active groups its records by type"
  );
  let rejected = page
    .records()
    .into_iter()
    .find(|record| record.status() == MemoryStatus::Rejected)
    .expect("the seed's rejected record");
  page.memory_tab(MemoryTab::Rejected);
  assert!(page.has(&format!("memory-record-{}", rejected.id())));
  assert!(
    !page.has(&format!("memory-group-{}", rejected.content().kind().name())),
    "Rejected is one list without type headings (`hxQhj`)"
  );
}

#[test]
fn memory_search_with_no_match_names_the_tab_that_holds_one_and_opens_it_keeping_the_search() {
  let mut page = Page::seeded("memory");
  page.memory_tab(MemoryTab::Active);
  page.window.set("memory-search", "kontur-ci-01");
  page.until("no match", |window| window.texts("memory-no-match").is_some());
  let empty = page.text("memory-no-match");
  assert!(empty.starts_with("No active records match “kontur-ci-01”"), "{empty}");
  assert!(
    empty.contains("Search finds words and parts of words, not other wording with the same meaning."),
    "{empty}"
  );
  assert_eq!(page.text("memory-no-match-note"), "1 match in Deprecated");
  assert_eq!(page.text("memory-open-tab-deprecated"), "Open Deprecated");
  assert!(page.has("memory-clear-search"));
  page.press_until("memory-open-tab-deprecated", "the Deprecated tab", |window| {
    window.texts("memory-tab-body-deprecated").is_some()
  });
  assert_eq!(
    page.window.navigation().route_now(),
    Route::Project(page.project, ProjectPage::Memory(Some(MemoryTab::Deprecated)))
  );
  assert_eq!(page.value("memory-search"), "kontur-ci-01", "the search is kept");
  let runner = page.record("CI runs on the self-hosted Windows runner").id();
  assert!(page.has(&format!("memory-record-{runner}")));
}

#[test]
fn memory_page_runs_past_no_edge_at_1440_or_960() {
  let mut page = Page::seeded("memory");
  for width in [1440_u32, 960] {
    page.window.tree.resize(width, 900);
    let right = f32::from(u16::try_from(width).expect("a window width"));
    page.project_memory();
    let past = page.past_right_edge("screen-memory", right);
    assert!(past.is_empty(), "Waiting at {width}: {past:?}");
    page.memory_tab(MemoryTab::Active);
    let engine = page.record("Kontur keeps its data in SQLite").id();
    page.window.press(&format!("memory-record-{engine}-select"));
    page.until("the details", |window| window.texts("memory-details").is_some());
    let past = page.past_right_edge("screen-memory", right);
    assert!(past.is_empty(), "Active with a record open at {width}: {past:?}");
    page.window.press(&format!("memory-record-{engine}-select"));
    page.memory_tab(MemoryTab::Settings);
    let past = page.past_right_edge("screen-memory", right);
    assert!(past.is_empty(), "Settings at {width}: {past:?}");
  }
}
