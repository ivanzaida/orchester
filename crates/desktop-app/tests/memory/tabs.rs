//! The Memory filter's statuses (`Memory filter` `cg7EL`): their order with and without waiting proposals (Y1, Y4),
//! the hub's Memory filter counting the active records, *Memory settings* at the line's end opening Project settings ›
//! Context (`S15` `BJQn0`), Approve and Reject moving a record to its status, a search with no match naming the status
//! that holds one and opening it with the search kept, and nothing past the window's right edge at 1440 and at
//! 1200 wide. Only *Active* groups its records by type; *Rejected* lists them in one group.

use orchester_app::routes::{MemoryTab, MemoryView, ProjectPage, ProjectSettingsTab, Route};
use orchester_core::memory::MemoryStatus;

use super::harness::Page;

const ALL_STATUSES_WAITING_FIRST: [&str; 5] = ["waiting", "active", "deprecated", "superseded", "rejected"];

#[test]
fn memory_tabs_lead_with_waiting_while_proposals_wait() {
  let mut page = Page::seeded("memory");
  page.project_memory();
  let (order, shown) = page.tabs();
  assert_eq!(order, ALL_STATUSES_WAITING_FIRST);
  assert_eq!(shown, "waiting", "the filter opens on what waits");
  assert_eq!(page.text("memory-tab-waiting"), "Waiting for approval 4");
  assert_eq!(page.text("memory-tab-waiting-count"), "4");
  assert_eq!(page.text("memory-tab-active"), "Active 10");
  assert_eq!(page.text("memory-tab-rejected"), "Rejected 1");
  // The hub's Memory filter counts the active records, and All does not count them (Question B `lmKu1`).
  page.until("the hub's counts", |window| {
    window.texts("local-hub-filter-memory-count").is_some()
  });
  assert_eq!(page.text("local-hub-filter-memory-count"), "10");
  let mut left = |id: &str| page.bounds_of(id).unwrap_or_else(|| panic!("{id} is laid out")).x;
  assert!(
    left("local-hub-filter-memory") > left("local-hub-filter-tools"),
    "after Tools"
  );
  assert!(
    left("local-hub-filter-memory") < left("local-hub-filter-archived"),
    "before Archived"
  );
}

/// *Memory settings* ends the statuses' line, right of the search, and opens Project settings › Context.
#[test]
fn memory_settings_link_ends_the_tabs_and_opens_project_settings_context() {
  let mut page = Page::seeded("memory");
  page.project_memory();
  assert_eq!(page.text("memory-settings-link"), "Memory settings");
  let mut left = |id: &str| page.bounds_of(id).unwrap_or_else(|| panic!("{id} is laid out")).x;
  assert!(left("memory-search") > left("memory-tab-rejected"));
  assert!(left("memory-settings-link") > left("memory-search"));
  let context = Route::Project(page.project, ProjectPage::Settings(ProjectSettingsTab::Context));
  page.press_until("memory-settings-link", "Project settings › Context", |window| {
    window.navigation().route_now() == context
  });
}

#[test]
fn memory_tabs_follow_active_while_nothing_waits() {
  let mut page = Page::seeded("tasks");
  page.project_memory();
  let (order, shown) = page.tabs();
  assert_eq!(order, ["active", "waiting", "deprecated", "superseded", "rejected"]);
  assert_eq!(shown, "active", "the filter opens on Active");
  assert!(!page.has("memory-tab-waiting-count"), "no count while nothing waits");
  assert_eq!(
    page.text("memory-tab-active"),
    "Active",
    "a status whose count is 0 shows its name only"
  );
  assert_eq!(page.text("memory-empty-title"), "No active records");
  page.memory_tab(MemoryTab::Waiting);
  assert_eq!(page.text("memory-empty-title"), "No records wait for approval");
  assert!(
    page
      .text("memory-empty-detail")
      .contains("Records agents propose for Kontur and its tasks"),
  );
}

#[test]
fn memory_waiting_approves_from_the_panel_and_the_record_moves_to_active() {
  let mut page = Page::seeded("memory");
  let typed = page.record("API handlers return typed errors").id();
  let old = page.record("Handlers map domain errors").id();
  page.memory_record(MemoryTab::Waiting, typed);
  page.press_until("memory-panel-approve", "the approval", |window| {
    window.text("memory-tab-waiting-count") == "3"
  });
  assert!(!page.has(&format!("memory-record-{typed}")), "it left Waiting");
  let (order, shown) = page.tabs();
  assert_eq!(
    order, ALL_STATUSES_WAITING_FIRST,
    "the order holds while the filter shows"
  );
  assert_eq!(shown, "waiting", "the status stays open where it is");
  assert!(page.has("memory-panel-approve"), "the next proposal is picked");
  assert_eq!(page.stored(typed).expect("kept").status(), MemoryStatus::Active);
  assert_eq!(page.stored(old).expect("kept").status(), MemoryStatus::Superseded);
  page.memory_tab(MemoryTab::Active);
  assert!(page.has(&format!("memory-record-{typed}")));
  page.memory_tab(MemoryTab::Superseded);
  assert!(page.has(&format!("memory-record-{old}")));
  assert_eq!(page.text("memory-tab-superseded"), "Superseded 2");
}

#[test]
fn memory_waiting_rejects_from_the_panel_and_the_record_moves_to_rejected() {
  let mut page = Page::seeded("memory");
  let typed = page.record("API handlers return typed errors").id();
  page.memory_record(MemoryTab::Waiting, typed);
  page.press_until("memory-panel-reject", "the rejection", |window| {
    window.text("memory-tab-waiting-count") == "3"
  });
  assert_eq!(page.stored(typed).expect("kept").status(), MemoryStatus::Rejected);
  assert_eq!(page.text("memory-tab-rejected"), "Rejected 2");
  page.memory_record(MemoryTab::Rejected, typed);
  assert!(page.has("memory-panel-delete"), "a rejected record is deleted only");
  assert!(!page.has("memory-panel-restore") && !page.has("memory-panel-edit"));
}

#[test]
fn memory_rejected_lists_one_group_while_active_groups_by_type() {
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
  assert_eq!(page.text("memory-group-rejected-title"), "Rejected");
  assert!(!page.has(&format!("memory-group-{}", rejected.content().kind().name())));
}

#[test]
fn memory_search_with_no_match_names_the_status_that_holds_one_and_opens_it_keeping_the_search() {
  let mut page = Page::seeded("memory");
  page.memory_tab(MemoryTab::Active);
  page.window.set("memory-search", "kontur-ci-01");
  page.until("no match", |window| window.texts("memory-no-match").is_some());
  let empty = page.text("memory-no-match");
  assert!(empty.starts_with("No active records match “kontur-ci-01”"), "{empty}");
  assert_eq!(page.text("memory-no-match-note"), "1 match in Deprecated");
  page.press_until("memory-open-tab-deprecated", "Deprecated", |window| {
    window.texts("memory-tab-body-deprecated").is_some()
  });
  assert_eq!(
    page.window.navigation().route_now(),
    Route::Project(page.project, ProjectPage::Memory(MemoryView::on(MemoryTab::Deprecated)))
  );
  assert_eq!(page.value("memory-search"), "kontur-ci-01", "the search is kept");
  let runner = page.record("CI runs on the self-hosted Windows runner").id();
  assert!(page.has(&format!("memory-record-{runner}")));
  page.window.set("memory-search", "");
  page.until("the search cleared", |window| window.texts("memory-no-match").is_none());
  assert!(page.has("memory-search"), "every status has its search");
}

#[test]
fn memory_page_runs_past_no_edge_at_1440_or_1200() {
  let mut page = Page::seeded("memory");
  let engine = page.record("Kontur keeps its data in SQLite").id();
  for width in [1440_u32, 1200] {
    page.window.tree.resize(width, 900);
    let right = f32::from(u16::try_from(width).expect("a window width"));
    page.project_memory();
    let past = page.past_right_edge("memory-body", right);
    assert!(past.is_empty(), "Waiting at {width}: {past:?}");
    page.memory_record(MemoryTab::Active, engine);
    let past = page.past_right_edge("memory-body", right);
    assert!(past.is_empty(), "a record at {width}: {past:?}");
  }
}
