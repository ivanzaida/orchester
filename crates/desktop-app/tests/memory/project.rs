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
  assert!(
    page.text("memory-tab-note").contains(
      "Agents get a record from their next invocation after you approve it. Until then no agent receives it."
    )
  );
  assert_eq!(page.text("memory-queue-title"), "Waiting for your approval");
  assert_eq!(page.text("memory-queue-count"), "4 · Oldest first");
  let typed = page.record("API handlers return typed errors");
  let proposal = format!("memory-proposal-{}", typed.id());
  assert_eq!(page.text(&format!("{proposal}-kind")), "Decision");
  let meta = page.text(&format!("{proposal}-scope"));
  assert!(meta.starts_with("Project · "), "{meta}");
  assert_eq!(
    page.text(&format!("{proposal}-supersedes")),
    "Supersedes 1 active record · api.errors by Reviewer"
  );
  let rate = page.record("GitHub's secondary rate limit");
  assert_eq!(page.text(&format!("memory-proposal-{}-kind", rate.id())), "Fact");
  let meta = page.text(&format!("memory-proposal-{}-scope", rate.id()));
  assert!(meta.starts_with("Task KON-16 · "), "{meta}");
  // The queue is listed whole, oldest first, as drawn (05): no search or filters on it.
  for filter in [
    "memory-filter-status",
    "memory-search",
    "memory-filter-type",
    "memory-filter-author",
    "memory-filter-tag",
  ] {
    assert!(!page.has(filter), "{filter} on the queue");
  }
  assert!(!page.has("memory-panel"), "no side panel");
}

#[test]
fn memory_project_page_lists_its_active_records_grouped_by_type() {
  let mut page = project();
  page.memory_tab(MemoryTab::Active);
  assert_eq!(page.text("memory-tab-active"), "Active 10");
  assert_eq!(page.text("memory-page-count"), "10 active · 4 waiting");
  let records = page.text("memory-records");
  // Each type is a group of the hub's rows, headed as the hub heads them (05b).
  for heading in ["Decisions", "Warnings", "Facts", "To do", "Summaries", "Observations"] {
    assert!(records.contains(heading), "{heading} in {records}");
  }
  assert!(
    !records.contains("ci.runner"),
    "a deprecated record shows in Deprecated only"
  );
  let engine = page.record("Kontur keeps its data in SQLite");
  let meta = page.text(&format!("memory-record-{}-meta", engine.id()));
  assert!(meta.contains("Planner · KON-12 · Run"), "{meta}");
  assert!(meta.contains("edited by you"), "{meta}");
  assert_eq!(
    page.text(&format!("memory-record-{}-history", engine.id())),
    "Supersedes 1 earlier version"
  );
  // One note under the search says what reaches agents and how the list is ordered (`aivVm` in 05b).
  assert!(
    page
      .text("memory-tab-note")
      .ends_with("Grouped by type · pinned first.")
  );
  assert!(page.has("memory-filter"), "one Filter");
  for filter in ["memory-filter-type", "memory-filter-author", "memory-filter-tag"] {
    assert!(!page.has(filter), "{filter} is in the Filter's menu");
  }
}

#[test]
fn memory_project_page_reviews_a_proposal_on_its_own_page_as_a_diff_of_the_active_record() {
  let mut page = project();
  let typed = page.record("API handlers return typed errors");
  let old = page.record("Handlers map domain errors");
  page.window.press(&format!("memory-proposal-{}-edit", typed.id()));
  let editor = format!("memory-editor-{}", typed.id());
  page.until("the review", |window| window.texts(&editor).is_some());
  // The review is a page of its own (05e): its crumb back to the tab, and no pills or queue under it.
  assert_eq!(page.text("memory-record-crumb"), "Memory");
  assert!(page.text("memory-record-title").starts_with("Review proposed"));
  assert!(!page.has("memory-tabs"), "no pills on the record's page");
  assert!(!page.has("memory-queue"), "no queue on the record's page");
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
  // Who proposed it and the person's edits head the diff box (`SGCy8` in 05e).
  assert!(
    page
      .text(&format!("{editor}-diff-head"))
      .ends_with("edited by you: the text")
  );
  assert!(page.has(&format!("{editor}-reject")));
  assert!(
    page.has(&format!("{editor}-importance-high")),
    "importance is a segmented control"
  );
  let (approve, frame) = (page.bounds(&format!("{editor}-save")), page.bounds(&editor));
  assert!(
    approve.x + approve.width <= frame.x + frame.width,
    "Approve fits in the footer: {approve:?} in {frame:?}"
  );
  page.window.press("memory-record-crumb");
  page.until("the queue again", |window| window.texts("memory-queue").is_some());
  assert!(!page.has(&editor));
}

#[test]
fn memory_project_page_opens_a_record_as_its_own_page_and_deprecates_it() {
  let mut page = project();
  page.memory_tab(MemoryTab::Active);
  let engine = page.record("Kontur keeps its data in SQLite").id();
  page.window.press(&format!("memory-record-{engine}-select"));
  page.until("the record's page", |window| window.texts("memory-details").is_some());
  assert!(page.has("memory-record-page"));
  assert!(
    page.has(&format!("memory-editor-{engine}")),
    "an active record opens in its editor"
  );
  assert_eq!(page.text("memory-details-source-title"), "Author and source");
  assert!(page.text("memory-details-history").contains("1 earlier version"));
  assert!(page.text("memory-details-source-hint").contains("KON-12 · Run"));
  page.until("the invocations it was given", |window| {
    window
      .texts("memory-details-given")
      .is_some_and(|texts| texts.join(" ").contains("No invocation was given it yet."))
  });
  assert!(page.has("memory-page-delete"));
  page.press_until("memory-page-deprecate", "the deprecation", |window| {
    window.texts("memory-record-page").is_none()
  });
  let deprecated = page.stored(engine).expect("kept");
  assert_eq!(deprecated.status(), MemoryStatus::Deprecated);
  assert!(!deprecated.is_pinned());
  page.memory_tab(MemoryTab::Deprecated);
  page.window.press(&format!("memory-record-{engine}-select"));
  page.until("its page", |window| window.texts("memory-record-page").is_some());
  assert!(page.has("memory-page-restore"));
  assert!(
    !page.has("memory-page-deprecate") && !page.has(&format!("memory-editor-{engine}")),
    "a deprecated record is restored or deleted"
  );
  page.window.press("memory-record-crumb");
  assert!(!page.has("memory-record-page"), "the crumb goes back to the list");
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
