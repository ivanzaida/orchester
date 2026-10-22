//! The project's Memory page's Settings tab (`U2teQq`) on the `memory` scenario, where both memories save agents'
//! records after approval: a saving card saves when chosen, a limit when its field is left, and a value that is not
//! one is said under its field and not saved. Only a person saves: an MCP client's input is refused.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::MemoryTab;
use orchester_core::memory::{MemorySettings, MemoryStore, WritePolicy};
use orchester_store::ProjectDb;

use super::harness::Page;

fn settings() -> Page {
  let mut page = Page::seeded("memory");
  page.memory_tab(MemoryTab::Settings);
  page.until("the settings", |window| window.texts("memory-settings").is_some());
  page
}

/// Waits until the stored settings are what `done` asks, and the tab says they are saved.
fn saved(page: &mut Page, what: &str, done: impl Fn(MemorySettings) -> bool) {
  let repo = page.repo.clone();
  page.until(what, |window| {
    let stored = ProjectDb::open(&repo)
      .and_then(|db| db.memory_settings())
      .expect("the settings");
    done(stored) && window.text("memory-settings-status-label") == "All changes saved"
  });
}

#[test]
fn memory_settings_tab_saves_a_saving_choice() {
  let mut page = settings();
  assert_eq!(
    page.text("memory-tab-note"),
    "How agents' records are kept in Kontur, and how much memory each agent gets."
  );
  assert_eq!(page.text("memory-settings-status-label"), "All changes saved");
  assert!(
    page
      .text("memory-settings-project-after-approval")
      .starts_with("Save after approval · default")
  );
  assert!(
    page
      .text("memory-settings-task-automatic")
      .starts_with("Save automatically · default")
  );
  page.window.press("memory-settings-task-automatic");
  saved(&mut page, "task memory saving automatically", |stored| {
    stored.task_writes == WritePolicy::Automatic
  });
  let stored = page.db().memory_settings().expect("the settings");
  assert_eq!(
    stored.project_writes,
    WritePolicy::AfterApproval,
    "the other memory keeps its choice"
  );
}

#[test]
fn memory_settings_tab_saves_a_limit_when_its_field_is_left() {
  let mut page = settings();
  assert_eq!(page.value("memory-settings-tokens-field"), "3,000");
  assert_eq!(
    page.text("memory-settings-tokens-field-read-back"),
    "= 3k tokens · 500 to 20,000 · default"
  );
  assert_eq!(
    page.text("memory-settings-records-field-read-back"),
    "1 to 50 · default 8"
  );
  page.focus("memory-settings-tokens-field");
  page.window.set("memory-settings-tokens-field", "5000");
  assert_eq!(page.text("memory-settings-status-label"), "Changes not saved");
  page.focus("memory-settings-records-field");
  saved(&mut page, "the new token limit", |stored| {
    stored.limits.tokens() == 5_000
  });
  let limits = page.db().memory_settings().expect("the settings").limits;
  assert_eq!(limits.records(), 8, "the other limit is kept");
  assert_eq!(page.value("memory-settings-tokens-field"), "5,000");
}

#[test]
fn memory_settings_tab_says_why_a_limit_is_not_saved() {
  let mut page = settings();
  page.focus("memory-settings-records-field");
  page.window.set("memory-settings-records-field", "99");
  page.focus("memory-settings-tokens-field");
  assert_eq!(
    page.text("memory-settings-records-field-note"),
    "Enter a whole number of records from 1 to 50."
  );
  assert_eq!(
    page.text("memory-settings-status-label"),
    "Not saved · fix the marked field"
  );
  assert_eq!(page.db().memory_settings().expect("the settings").limits.records(), 8);
}

#[test]
fn memory_settings_tab_refuses_an_mcp_clients_input() {
  let mut page = settings();
  let before = page.db().memory_settings().expect("the settings");
  assert_eq!(before.project_writes, WritePolicy::AfterApproval);
  with_synthetic_input(|| page.window.press("memory-settings-project-automatic"));
  page.until("the person-only note", |window| {
    window.texts("memory-settings-person-only").is_some()
  });
  assert!(
    page
      .text("memory-settings-person-only")
      .contains("Only a person changes these settings")
  );
  page.focus("memory-settings-tokens-field");
  page.window.set("memory-settings-tokens-field", "20000");
  with_synthetic_input(|| page.focus("memory-settings-records-field"));
  assert_eq!(page.text("memory-settings-status-label"), "Changes not saved");
  assert_eq!(
    page.db().memory_settings().expect("the settings"),
    before,
    "neither automatic saving nor a higher limit is stored"
  );
}
