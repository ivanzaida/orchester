//! The project's Memory page (`Memory · project page` `xY2Uj`) on the `memory` scenario: every proposal in one queue,
//! the project's records grouped by type, the *Project memory* panel, and a chosen record's inspector with its
//! actions (`Memory · record selected` `NbMOn`).

use orchester_core::memory::MemoryStatus;

use super::harness::Page;

fn project() -> Page {
  let mut page = Page::seeded("memory");
  page.project_memory();
  page
}

#[test]
fn memory_project_page_queues_every_proposal_and_lists_the_projects_records() {
  let mut page = project();
  assert_eq!(page.text("memory-queue-count"), "4");
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
  let records = page.text("memory-records");
  for heading in ["DECISIONS", "WARNINGS", "FACTS", "TO DO", "SUMMARIES", "OBSERVATIONS"] {
    assert!(records.contains(heading), "{heading} in {records}");
  }
  assert!(
    !records.contains("ci.runner"),
    "a deprecated record shows under Status · Deprecated only"
  );
  assert_eq!(page.text("memory-records-count"), "10 active");
  assert_eq!(page.text("memory-panel-active"), "Active 10 · given to agents");
  assert_eq!(
    page.text("memory-panel-waiting"),
    "Waiting for approval 4 · 1 project, 3 task"
  );
  let engine = page.record("Kontur keeps its data in SQLite");
  let meta = page.text(&format!("memory-record-{}-meta", engine.id()));
  assert!(meta.starts_with("Planner · KON-12 · Run"), "{meta}");
  assert!(meta.contains("edited by you"), "{meta}");
  assert_eq!(
    page.text(&format!("memory-record-{}-history", engine.id())),
    "Supersedes 1 earlier version"
  );
}

#[test]
fn memory_project_page_approves_from_the_queue() {
  let mut page = project();
  let typed = page.record("API handlers return typed errors").id();
  let old = page.record("Handlers map domain errors").id();
  page.press_until(&format!("memory-proposal-{typed}-approve"), "the approval", |window| {
    window.text("memory-queue-count") == "3"
  });
  assert_eq!(page.stored(typed).expect("kept").status(), MemoryStatus::Active);
  assert_eq!(page.stored(old).expect("kept").status(), MemoryStatus::Superseded);
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
  let proposed = page.text(&format!("{editor}-added-meta"));
  assert!(proposed.starts_with("Proposed by "), "{proposed}");
  assert!(!proposed.contains("edited by you"), "{proposed}");
  page
    .window
    .set(&format!("{editor}-text"), "API handlers return typed errors.");
  assert_eq!(
    page.text(&format!("{editor}-added-text")),
    "API handlers return typed errors."
  );
  assert!(
    page
      .text(&format!("{editor}-added-meta"))
      .ends_with("edited by you: the text")
  );
  assert_eq!(
    page.text(&format!("{editor}-reject-note")),
    "Kept, never given or restored."
  );
  let link = page.text(&format!("{editor}-source-link"));
  assert!(
    link.starts_with("Show in KON-") && link.ends_with("'s Conversation ↗"),
    "{link}"
  );
  let (approve, frame) = (page.bounds(&format!("{editor}-save")), page.bounds(&editor));
  assert!(
    approve.x + approve.width <= frame.x + frame.width,
    "Approve fits in the footer: {approve:?} in {frame:?}"
  );
}

#[test]
fn memory_project_page_says_no_records_match_in_the_middle_of_the_list() {
  let mut page = project();
  page.window.set("memory-search", "kontur-ci-01");
  page.until("no match", |window| window.texts("memory-no-match").is_some());
  assert!(page.has("memory-no-match-glyph"));
  let (title, list) = (page.bounds("memory-no-match-title"), page.bounds("memory-records"));
  let middle = title.x + title.width / 2.0 - (list.x + list.width / 2.0);
  assert!(middle.abs() < 2.0, "centered: {title:?} in {list:?}");
}

#[test]
fn memory_project_page_inspects_a_chosen_record_and_deprecates_it() {
  let mut page = project();
  let engine = page.record("Kontur keeps its data in SQLite").id();
  page.window.press(&format!("memory-record-{engine}-select"));
  page.until("the inspector", |window| window.texts("memory-inspector").is_some());
  let inspector = page.text("memory-inspector");
  assert!(inspector.starts_with("Decision db.engine"), "{inspector}");
  assert!(page.text("memory-inspector-status").starts_with("Active Pinned"));
  assert!(page.text("memory-inspector-history").contains("1 earlier version"));
  assert!(page.text("memory-inspector-source").contains("KON-12 · Run"));
  page.until("the invocations it was given", |window| {
    window
      .texts("memory-inspector-given")
      .is_some_and(|texts| texts.join(" ").contains("No invocation was given it yet."))
  });
  page.press_until("memory-inspector-deprecate", "the deprecation", |window| {
    window
      .texts("memory-inspector-status")
      .is_some_and(|texts| texts.join(" ").starts_with("Deprecated"))
  });
  let deprecated = page.stored(engine).expect("kept");
  assert_eq!(deprecated.status(), MemoryStatus::Deprecated);
  assert!(!deprecated.is_pinned());
  assert!(page.has("memory-inspector-restore"));
  assert!(
    !page.has("memory-inspector-edit"),
    "a deprecated record is restored or deleted"
  );
}

#[test]
fn memory_project_page_writes_a_new_project_record() {
  let mut page = project();
  page.window.press("memory-new");
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

#[test]
fn memory_project_page_settings_are_unavailable_until_their_page_is_built() {
  let mut page = project();
  assert!(page.has("memory-panel-settings-link"));
  let lock = page
    .window
    .tree
    .get_element_by_id("memory-settings-icon")
    .and_then(|element| element.text_content().map(str::to_owned));
  assert_eq!(lock, Some(orchester_app::ui::icon::Icon::Lock.glyph().to_string()));
}
