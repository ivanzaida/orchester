//! The Memory screens' other states on the `memory` scenario: a reading that failed with *Try again* (`AlQy0`),
//! records that couldn't be read (`hsIEe`, `zGBYl`), the project's search with no results (`z4t51u`), a restore
//! refused while the key names another active record (`rKIl1`), a save refused because it holds a secret's value,
//! a change refused because the record changed meanwhile, and that only a person changes memory: synthetic input
//! (an MCP client driving the window) is refused in place.

use lurq::app::events::with_synthetic_input;
use orchester_core::memory::MemoryStatus;

use super::harness::Page;

/// A key over two lines, which no record can have: the row can't be read back.
const TWO_LINE_KEY: &str = "UPDATE memories SET key = 'two' || char(10) || 'lines' WHERE id = ?1";

#[test]
fn memory_task_tab_says_it_couldnt_read_and_tries_again() {
  let mut page = Page::seeded("memory");
  // The task's page counts the project's waiting proposals from `memories`; only the tab reads the settings.
  page.edit_outside("ALTER TABLE memory_settings RENAME TO memory_settings_away", None);
  page.task_memory("KON-12");
  let failed = page.text("memory-load-error");
  assert!(failed.starts_with("Couldn't read the memory of KON-12"), "{failed}");
  page.edit_outside("ALTER TABLE memory_settings_away RENAME TO memory_settings", None);
  page.press_until("memory-retry", "the memory read again", |window| {
    window.texts("memory-load-error").is_none() && window.texts("memory-count").is_some()
  });
  assert_eq!(page.text("memory-count"), "10 active · 2 waiting");
}

#[test]
fn memory_project_page_says_it_couldnt_read_and_tries_again() {
  let mut page = Page::seeded("memory");
  page.edit_outside("ALTER TABLE memories RENAME TO memories_away", None);
  page.project_memory();
  assert!(page.text("memory-load-error").starts_with("Memory couldn't be read"));
  page.edit_outside("ALTER TABLE memories_away RENAME TO memories", None);
  page.press_until("memory-load-error-retry", "the memory read again", |window| {
    window.texts("memory-body").is_some()
  });
  assert_eq!(page.text("memory-records-count"), "10 active");
}

#[test]
fn memory_unreadable_records_are_left_out_and_said() {
  let mut page = Page::seeded("memory");
  let layout = page.record("Domain code lives in crates/core").id();
  assert_eq!(page.edit_outside(TWO_LINE_KEY, Some(layout)), 1);
  page.project_memory();
  assert!(
    page
      .text("memory-unreadable")
      .starts_with("1 record couldn't be read and is left out")
  );
  assert_eq!(page.text("memory-records-count"), "9 active · 1 left out");
  assert!(!page.has(&format!("memory-record-{layout}")));
  page.task_memory("KON-12");
  assert!(
    page
      .text("memory-unreadable")
      .starts_with("1 record couldn't be read and is left out")
  );
}

#[test]
fn memory_project_search_with_no_results_names_the_status_it_lists() {
  let mut page = Page::seeded("memory");
  page.project_memory();
  page.window.set("memory-search", "kontur-ci-01");
  page.until("no match", |window| window.texts("memory-no-match").is_some());
  let empty = page.text("memory-no-match");
  assert!(empty.starts_with("No active records match “kontur-ci-01”"), "{empty}");
  assert!(
    empty.contains("Search finds words and parts of words, not other wording with the same meaning."),
    "{empty}"
  );
  assert!(empty.contains("1 match among deprecated records"), "{empty}");
  assert!(page.has("memory-clear-search"));
  page.press_until("memory-show-status", "the deprecated records", |window| {
    window.texts("memory-no-match").is_none()
  });
  assert_eq!(page.text("memory-filter-status"), "Status · Deprecated");
  let runner = page.record("CI runs on the self-hosted Windows runner").id();
  assert!(page.has(&format!("memory-record-{runner}")));
}

#[test]
fn memory_restore_is_refused_while_the_key_names_another_active_record() {
  let mut page = Page::seeded("memory");
  let runner = page.record("CI runs on the self-hosted Windows runner").id();
  let times = page.record("macOS CI builds take about 9 minutes").id();
  assert_eq!(
    page.edit_outside("UPDATE memories SET key = 'ci.runner' WHERE id = ?1", Some(times)),
    1
  );
  page.project_memory();
  page.window.press("memory-filter-status");
  page.window.press("memory-filter-status-menu-3");
  let row = format!("memory-record-{runner}");
  page.until("the deprecated records", |window| window.texts(&row).is_some());
  page.hover(&row);
  page.press_until(&format!("{row}-restore"), "the refusal", |window| {
    window.texts(&format!("{row}-error")).is_some()
  });
  assert_eq!(
    page.text(&format!("{row}-error")),
    "Can't restore: ci.runner already names an active record. Deprecate that one first."
  );
  assert_eq!(page.stored(runner).expect("kept").status(), MemoryStatus::Deprecated);
}

#[test]
fn memory_a_record_holding_a_secrets_value_is_not_saved() {
  let mut page = Page::seeded("memory");
  page.secret("WEBHOOK_TOKEN", "whk-0123456789");
  page.task_memory("KON-12");
  page.window.press("memory-new");
  page
    .window
    .set("memory-editor-new-text", "The token is whk-0123456789.");
  page.press_until("memory-editor-new-save", "the refusal", |window| {
    window.texts("memory-editor-new-error").is_some()
  });
  assert!(
    page
      .text("memory-editor-new-error")
      .starts_with("The record holds the value of secret WEBHOOK_TOKEN, so it wasn't saved.")
  );
  assert!(
    page.has("memory-editor-new"),
    "the editor stays open with what was written"
  );
  assert!(
    page
      .records()
      .iter()
      .all(|record| !record.content().text().contains("whk-0123456789")),
    "nothing was stored"
  );
}

#[test]
fn memory_a_change_of_a_record_changed_meanwhile_is_refused_and_read_again() {
  let mut page = Page::seeded("memory");
  page.task_memory("KON-12");
  let id = page.record("Exponential backoff from 500 ms").id();
  page.edit_outside(
    "UPDATE memories SET text = 'Exponential backoff from 250 ms.', revision = revision + 1 WHERE id = ?1",
    Some(id),
  );
  let row = format!("memory-record-{id}");
  page.hover(&row);
  page.press_until(&format!("{row}-pin"), "the refusal", |window| {
    window.texts(&format!("{row}-error")).is_some()
  });
  assert!(
    page
      .text(&format!("{row}-error"))
      .starts_with("The record changed since this page read it, so nothing was saved.")
  );
  page.until("the record read again", |window| {
    window.text(&format!("{row}-text")) == "Exponential backoff from 250 ms."
  });
  assert!(!page.stored(id).expect("kept").is_pinned());
}

#[test]
fn memory_synthetic_input_approves_nothing_and_says_only_a_person_changes_memory() {
  let mut page = Page::seeded("memory");
  page.project_memory();
  let typed = page.record("API handlers return typed errors").id();
  let proposal = format!("memory-proposal-{typed}");
  with_synthetic_input(|| page.window.press(&format!("{proposal}-approve")));
  page.until("the refusal", |window| {
    window.texts(&format!("{proposal}-person-only")).is_some()
  });
  assert!(
    page
      .text(&format!("{proposal}-person-only"))
      .starts_with("Only a person changes memory here")
  );
  assert_eq!(page.stored(typed).expect("kept").status(), MemoryStatus::Proposed);
  assert_eq!(page.text("memory-queue-count"), "4");
  page.press_until(&format!("{proposal}-approve"), "a person's approval", |window| {
    window.text("memory-queue-count") == "3"
  });
  assert_eq!(page.stored(typed).expect("kept").status(), MemoryStatus::Active);
}

#[test]
fn memory_synthetic_input_deletes_nothing() {
  let mut page = Page::seeded("memory");
  page.task_memory("KON-12");
  let id = page.record("Exponential backoff from 500 ms").id();
  let row = format!("memory-record-{id}");
  page.hover(&row);
  page.window.press(&format!("{row}-delete"));
  let confirm = format!("memory-delete-{id}");
  page.until("the confirmation", |window| {
    window.texts(&format!("{confirm}-confirm")).is_some()
  });
  with_synthetic_input(|| page.window.press(&format!("{confirm}-confirm")));
  page.until("the refusal", |window| {
    window.texts(&format!("{confirm}-person-only")).is_some()
  });
  assert!(page.stored(id).is_some(), "nothing was deleted");
  assert!(page.has(&confirm), "the confirmation stays open");
}
