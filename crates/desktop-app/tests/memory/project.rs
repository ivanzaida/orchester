//! The Agents hub's Memory filter (`Section Y · Memory` `Xxl1u`) on the `memory` scenario: every proposal under
//! *Waiting for approval* with the first picked and *Approve*, *Edit* and *Reject* pinned (Y1 `tsfxN`), the project's
//! records grouped by type in *Active* with the picked one's panel (Y2 `oIgSL`), a record edited and a proposal
//! reviewed in the panel while the list stays (Y3 `hnMhi`), a deprecated record restored (Y6 `tcffY`), *Delete…* asking
//! in a modal, and a new record written from the head.

use orchester_app::routes::{MemoryTab, MemoryView, ProjectPage, Route};
use orchester_core::memory::{MemoryScope, MemoryStatus};

use super::harness::Page;

fn project() -> Page {
  let mut page = Page::seeded("memory");
  page.project_memory();
  page
}

#[test]
fn memory_hub_lists_every_proposal_under_waiting_with_the_first_picked() {
  let mut page = project();
  assert!(page.has("local-hub-filter-memory"), "Memory is the hub's filter");
  assert_eq!(page.text("memory-tab-waiting-count"), "4");
  assert!(
    page.text("memory-tab-note").contains(
      "Agents get a record from their next invocation after you approve it. Until then no agent receives it."
    )
  );
  assert_eq!(page.text("memory-queue-title"), "Waiting for your approval");
  assert_eq!(page.text("memory-queue-count"), "4 · Oldest first");
  let rate = page.record("GitHub's secondary rate limit");
  let line = page.text(&format!("memory-record-{}-line", rate.id()));
  assert!(line.starts_with("Task KON-16 · "), "{line}");
  // The first proposal is the panel's, with its actions pinned (Y1).
  assert!(page.text("team-hub-panel-title").starts_with("Review proposed"));
  for action in ["memory-panel-approve", "memory-panel-edit", "memory-panel-reject"] {
    assert!(page.has(action), "{action} pinned");
  }
  assert!(!page.has("memory-panel-delete"), "a proposal is rejected, not deleted");
  let typed = page.record("API handlers return typed errors");
  page.memory_record(MemoryTab::Waiting, typed.id());
  let line = page.text(&format!("memory-record-{}-line", typed.id()));
  assert!(line.starts_with("Project · "), "{line}");
  assert!(
    page
      .text("memory-panel-notice")
      .starts_with("Approving replaces the active decision api.errors"),
    "what approving replaces"
  );
  assert_eq!(
    page.text("memory-source-key-value"),
    typed.content().key().expect("keyed").as_str()
  );
  assert!(
    page.has("memory-person-only"),
    "the list says only a person changes memory"
  );
}

#[test]
fn memory_hub_lists_its_active_records_grouped_by_type() {
  let mut page = project();
  page.memory_tab(MemoryTab::Active);
  assert_eq!(page.text("memory-tab-active"), "Active 11");
  for kind in ["decision", "warning", "fact", "todo", "summary", "observation"] {
    assert!(page.has(&format!("memory-group-{kind}")), "{kind} grouped");
  }
  let records = page.text("memory-records");
  assert!(
    !records.contains("ci.runner"),
    "a deprecated record shows in Deprecated only"
  );
  let engine = page.record("Kontur keeps its data in SQLite");
  let line = page.text(&format!("memory-record-{}-line", engine.id()));
  assert!(line.contains("Planner · Run"), "{line}");
  assert!(line.contains("edited by you"), "{line}");
  assert!(
    page
      .text("memory-tab-note")
      .ends_with("Grouped by type · pinned first.")
  );
  // The project's handoffs are a group of their own (`Section · Handoffs` `OiuKs` in Y2).
  let handoff = page.record("02 Plan passed: plan approved");
  assert!(page.has("memory-group-handoff"));
  assert!(page.has(&format!("memory-record-{}", handoff.id())));
  assert!(page.has("memory-search"), "the search sits beside the statuses");
  // A record no invocation was given has no *Given to agents* (Y6 `tcffY` draws none).
  let layout = page.record("Domain code lives in crates/core").id();
  page.memory_record(MemoryTab::Active, layout);
  page.until("the panel", |window| window.texts("memory-panel-text").is_some());
  assert!(!page.has("memory-given"));
  assert!(page.has("memory-settings-link"));
}

/// The meta line ends in an ellipsis before the tags, never under them (Y1 `PdesS`).
#[test]
fn memory_hub_row_cuts_its_line_before_its_tags() {
  let mut page = project();
  page.window.tree.resize(1440, 900);
  page.memory_tab(MemoryTab::Active);
  let tagged = page
    .records()
    .into_iter()
    .find(|record| {
      record.status() == MemoryStatus::Active
        && record.scope() == MemoryScope::Project
        && record.content().tags().iter().next().is_some()
    })
    .expect("an active tagged record");
  let row = format!("memory-record-{}", tagged.id());
  let (line, tag) = (page.bounds(&format!("{row}-line")), page.bounds(&format!("{row}-tag")));
  assert!(line.x + line.width <= tag.x, "{line:?} runs under {tag:?}");
  let (name, row_bounds) = (page.bounds(&format!("{row}-name")), page.bounds(&row));
  assert!(
    name.x + name.width <= row_bounds.x + row_bounds.width,
    "the name keeps in its row"
  );
}

#[test]
fn memory_hub_reviews_a_proposal_in_the_panel_beside_the_list() {
  let mut page = project();
  let typed = page.record("API handlers return typed errors");
  let old = page.record("Handlers map domain errors");
  page.memory_record(MemoryTab::Waiting, typed.id());
  page.window.press("memory-panel-edit");
  let editor = format!("memory-editor-{}", typed.id());
  page.until("the review", |window| window.texts(&editor).is_some());
  assert!(page.has("memory-list"), "the list stays");
  assert!(page.text("team-hub-panel-title").starts_with("Review proposed"));
  assert_eq!(
    page.text(&format!("{editor}-removed-text")),
    old.content().text(),
    "the active text on the removed line"
  );
  assert_eq!(page.text(&format!("{editor}-added-text")), typed.content().text());
  page
    .window
    .set(&format!("{editor}-text"), "API handlers return typed errors.");
  assert!(
    page
      .text(&format!("{editor}-diff-head"))
      .ends_with("edited by you: the text")
  );
  assert!(page.has(&format!("{editor}-reject")));
  assert!(page.has(&format!("{editor}-importance")), "importance is a select");
  page.window.press(&format!("{editor}-cancel"));
  page.until("the record again", |window| window.texts(&editor).is_none());
  assert!(page.has("memory-panel-approve"));
}

#[test]
fn memory_hub_edits_a_record_in_the_panel_and_deprecates_and_restores_it() {
  let mut page = project();
  let engine = page.record("Kontur keeps its data in SQLite").id();
  page.memory_record(MemoryTab::Active, engine);
  assert_eq!(page.text("team-hub-panel-kind"), "Memory");
  assert!(page.has("memory-source"));
  // It was given to two invocations (the seed's KON-12 and KON-16), the latest listed (Y2 `kIRpz`).
  page.until("the invocations it was given", |window| {
    window.texts("memory-given").is_some()
  });
  assert_eq!(page.text("memory-given-count"), "2 invocations");
  assert!(page.has("memory-given-invocation-0") && page.has("memory-given-invocation-1"));
  page.window.press("memory-panel-edit");
  let editor = format!("memory-editor-{engine}");
  page.until("the editor in the panel", |window| window.texts(&editor).is_some());
  assert!(page.has("memory-list"), "the list stays (Y3)");
  assert!(page.text("team-hub-panel-title").starts_with("Edit"));
  assert!(page.text(&format!("{editor}-record")).contains("1 earlier version"));
  page.window.press(&format!("{editor}-cancel"));
  page.until("the record again", |window| {
    window.texts("memory-panel-deprecate").is_some()
  });
  page.press_until("memory-panel-deprecate", "the deprecation", |window| {
    window.texts(&format!("memory-record-{engine}")).is_none()
  });
  assert_eq!(page.stored(engine).expect("kept").status(), MemoryStatus::Deprecated);
  page.memory_record(MemoryTab::Deprecated, engine);
  assert!(
    page
      .text("memory-panel-notice")
      .starts_with("Deprecated: no agent is given it")
  );
  assert!(
    !page.has("memory-panel-deprecate"),
    "a deprecated record is restored or deleted"
  );
  assert!(page.has("memory-panel-delete"));
  page.press_until("memory-panel-restore", "the restore", |window| {
    window.texts(&format!("memory-record-{engine}")).is_none()
  });
  assert_eq!(page.stored(engine).expect("kept").status(), MemoryStatus::Active);
}

#[test]
fn memory_hub_deletes_a_record_after_asking_in_a_modal() {
  let mut page = project();
  let engine = page.record("Kontur keeps its data in SQLite").id();
  page.memory_record(MemoryTab::Active, engine);
  page.window.press("memory-panel-delete");
  let modal = format!("memory-delete-{engine}");
  page.until("the question", |window| {
    window.texts(&format!("{modal}-title")).is_some()
  });
  assert_eq!(page.text(&format!("{modal}-title")), "Delete this record?");
  assert!(
    page.has(&format!("{modal}-deprecate")),
    "an active record can be deprecated instead"
  );
  page.window.press(&format!("{modal}-cancel"));
  page.until("the question closed", |window| window.texts(&modal).is_none());
  assert!(page.stored(engine).is_some());
  page.window.press("memory-panel-delete");
  page.until("the question again", |window| window.texts(&modal).is_some());
  page.press_until(&format!("{modal}-confirm"), "the deletion", |window| {
    window.texts(&format!("memory-record-{engine}")).is_none()
  });
  assert!(page.stored(engine).is_none());
}

#[test]
fn memory_hub_writes_a_new_project_record_from_any_status() {
  let mut page = project();
  page.press_until("memory-new", "Active with the editor", |window| {
    window.texts("memory-tab-body-active").is_some() && window.texts("memory-editor-new").is_some()
  });
  assert_eq!(
    page.window.navigation().route_now(),
    Route::Project(page.project, ProjectPage::Memory(MemoryView::on(MemoryTab::Active)))
  );
  assert!(page.has("memory-list"), "the list stays beside the editor");
  assert!(
    !page.has("memory-editor-new-save-to-field"),
    "the hub saves to the project"
  );
  page
    .window
    .set("memory-editor-new-text", "Release notes live in CHANGELOG.md.");
  page.press_until("memory-editor-new-save", "the new record", |window| {
    window.texts("memory-editor-new").is_none()
  });
  let created = page.record("Release notes live in CHANGELOG.md.");
  assert_eq!(created.scope(), MemoryScope::Project);
  assert_eq!(created.status(), MemoryStatus::Active);
}
