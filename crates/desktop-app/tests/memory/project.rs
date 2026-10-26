//! The project's Memory page (`Memory with tabs` `wMr98`) on the `memory` scenario: every proposal in the *Waiting
//! for approval* tab (`CIThk`), the project's records grouped by type in *Active* with a record's details opened in
//! place (`TeKGM`), and a new record written from the header.

use orchester_app::routes::MemoryTab;
use orchester_core::memory::MemoryStatus;

use super::harness::Page;

fn project() -> Page {
  let mut page = Page::seeded("memory");
  page.project_memory();
  page
}

#[test]
fn memory_project_page_lists_every_proposal_in_its_waiting_tab() {
  let mut page = project();
  assert_eq!(page.text("memory-tab-waiting-count"), "4");
  assert_eq!(
    page.text("memory-tab-note"),
    "Agents get a record from their next invocation after you approve it. Until then no agent receives it."
  );
  assert_eq!(page.text("memory-tab-order"), "Oldest first");
  let typed = page.record("API handlers return typed errors");
  let proposal = format!("memory-proposal-{}", typed.id());
  assert_eq!(page.text(&format!("{proposal}-scope")), "Project · Decision");
  assert_eq!(
    page.text(&format!("{proposal}-supersedes")),
    "Supersedes 1 active record · api.errors by Reviewer"
  );
  let rate = page.record("GitHub's secondary rate limit");
  assert_eq!(
    page.text(&format!("memory-proposal-{}-scope", rate.id())),
    "Task KON-16 · Fact"
  );
  assert!(!page.has("memory-filter-status"), "the tab is the status");
  for filter in [
    "memory-search",
    "memory-filter-type",
    "memory-filter-author",
    "memory-filter-tag",
  ] {
    assert!(page.has(filter), "{filter} in the tab");
  }
  assert!(!page.has("memory-panel"), "no side panel");
}

#[test]
fn memory_project_page_lists_its_active_records_grouped_by_type() {
  let mut page = project();
  page.memory_tab(MemoryTab::Active);
  assert_eq!(page.text("memory-tab-active"), "Active 10");
  let records = page.text("memory-records");
  for heading in ["DECISIONS", "WARNINGS", "FACTS", "TO DO", "SUMMARIES", "OBSERVATIONS"] {
    assert!(records.contains(heading), "{heading} in {records}");
  }
  assert!(
    !records.contains("ci.runner"),
    "a deprecated record shows in Deprecated only"
  );
  let engine = page.record("Kontur keeps its data in SQLite");
  let meta = page.text(&format!("memory-record-{}-meta", engine.id()));
  assert!(meta.starts_with("Planner · KON-12 · Run"), "{meta}");
  assert!(meta.contains("edited by you"), "{meta}");
  assert_eq!(
    page.text(&format!("memory-record-{}-history", engine.id())),
    "Supersedes 1 earlier version"
  );
  assert_eq!(page.text("memory-tab-order"), "Grouped by type · pinned first");
}

#[test]
fn memory_project_page_reviews_a_proposal_as_a_diff_of_the_active_record() {
  let mut page = project();
  let typed = page.record("API handlers return typed errors");
  let old = page.record("Handlers map domain errors");
  page.window.press(&format!("memory-proposal-{}-edit", typed.id()));
  let editor = format!("memory-editor-{}", typed.id());
  page.until("the review", |window| window.texts(&editor).is_some());
  assert_eq!(
    page.text(&format!("{editor}-removed-text")),
    old.content().text(),
    "the active text on the removed line"
  );
  let was = page.text(&format!("{editor}-removed-meta"));
  assert!(was.starts_with("Active · Reviewer"), "{was}");
  assert!(was.contains("given to "), "{was}");
  assert_eq!(page.text(&format!("{editor}-added-text")), typed.content().text());
  page
    .window
    .set(&format!("{editor}-text"), "API handlers return typed errors.");
  assert!(
    page
      .text(&format!("{editor}-added-meta"))
      .ends_with("edited by you: the text")
  );
  assert_eq!(
    page.text(&format!("{editor}-reject-note")),
    "Kept, never given or restored."
  );
  let (approve, frame) = (page.bounds(&format!("{editor}-save")), page.bounds(&editor));
  assert!(
    approve.x + approve.width <= frame.x + frame.width,
    "Approve fits in the footer: {approve:?} in {frame:?}"
  );
}

#[test]
fn memory_project_page_opens_a_records_details_in_place_and_deprecates_it() {
  let mut page = project();
  page.memory_tab(MemoryTab::Active);
  let engine = page.record("Kontur keeps its data in SQLite").id();
  page.window.press(&format!("memory-record-{engine}-select"));
  page.until("the details", |window| window.texts("memory-details").is_some());
  assert!(page.has(&format!("memory-record-{engine}-open")));
  assert_eq!(page.text("memory-details-source-title"), "Author and source");
  assert!(page.text("memory-details-history").contains("1 earlier version"));
  assert!(page.text("memory-details-source-value").contains("KON-12 · Run"));
  page.until("the invocations it was given", |window| {
    window
      .texts("memory-details-given")
      .is_some_and(|texts| texts.join(" ").contains("No invocation was given it yet."))
  });
  assert_eq!(page.text("memory-details-actions"), "Edit Unpin Deprecate Delete");
  page.press_until("memory-details-deprecate", "the deprecation", |window| {
    window.texts(&format!("memory-record-{engine}")).is_none()
  });
  let deprecated = page.stored(engine).expect("kept");
  assert_eq!(deprecated.status(), MemoryStatus::Deprecated);
  assert!(!deprecated.is_pinned());
  page.memory_tab(MemoryTab::Deprecated);
  page.window.press(&format!("memory-record-{engine}-select"));
  page.until("its details", |window| window.texts("memory-details").is_some());
  assert!(page.has("memory-details-restore"));
  assert!(
    !page.has("memory-details-edit"),
    "a deprecated record is restored or deleted"
  );
  page.window.press(&format!("memory-record-{engine}-select"));
  assert!(!page.has("memory-details"), "a second press closes them");
}

#[test]
fn memory_project_page_writes_a_new_project_record_from_any_tab() {
  let mut page = project();
  page.window.press("memory-new");
  page.until("the Active tab with the editor", |window| {
    window.texts("memory-tab-body-active").is_some() && window.texts("memory-editor-new").is_some()
  });
  assert!(
    !page.has("memory-editor-new-save-to-field"),
    "the project's page saves to the project"
  );
  page
    .window
    .set("memory-editor-new-text", "Release notes live in CHANGELOG.md.");
  page.press_until("memory-editor-new-save", "the new record", |window| {
    window.texts("memory-editor-new").is_none()
  });
  let created = page.record("Release notes live in CHANGELOG.md.");
  assert_eq!(created.scope(), orchester_core::memory::MemoryScope::Project);
  assert_eq!(created.status(), MemoryStatus::Active);
}
