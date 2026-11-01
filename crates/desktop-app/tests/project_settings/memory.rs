//! Context › Memory (`S04` `bdcD4`): after Code index, a select per memory whose default option says so, the record and
//! token limits with their units and read-backs, and the note under the box. A choice saves when it is made, a limit
//! when its field is left, as the Memory page's Settings tab saves them. Only a person saves: an MCP client's input is
//! refused and said, and nothing is stored.
use lurq::app::events::with_synthetic_input;
use orchester_core::memory::{MemorySettings, MemoryStore, WritePolicy};

use super::harness::Settings;

fn context() -> Settings {
  let mut page = Settings::new(false);
  page.tab("context", "settings-memory-task-select");
  page
}

fn stored(page: &Settings) -> MemorySettings {
  page.db().memory_settings().expect("the memory settings")
}

/// Moves the focus to `id`, as clicking into another field does.
fn focus(page: &mut Settings, id: &str) {
  page
    .window
    .tree
    .get_element_by_id_mut(id)
    .unwrap_or_else(|| panic!("{id} is shown"))
    .focus();
  page.window.pass();
}

#[test]
fn project_settings_context_memory_shows_its_group_after_the_code_index() {
  let mut page = context();
  page.until("the code index", |p| p.exists("settings-index-section"));
  assert_eq!(page.text("settings-memory-title"), "Memory");
  assert_eq!(page.text("settings-memory-only-you"), "Only you, here in the window");
  assert!(page.text("settings-memory-task-row").contains("Task memory"));
  assert_eq!(
    page.text("settings-memory-project-hint"),
    "Facts and decisions agents record for every task of Kontur"
  );
  assert_eq!(page.value("settings-memory-tokens-field"), "3,000");
  assert_eq!(
    page.text("settings-memory-tokens-field-read-back"),
    "= 3k tokens · default"
  );
  assert_eq!(page.text("settings-memory-records-field-read-back"), "default 8");
  assert_eq!(
    page.text("settings-memory-note-text"),
    "Records stay on the Memory page. Changes apply from each agent's next invocation."
  );
  let top = |page: &mut Settings, id: &str| {
    page
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
      .y
  };
  let (index, memory) = (
    top(&mut page, "settings-index-section-title"),
    top(&mut page, "settings-memory-title"),
  );
  assert!(index < memory, "{index} < {memory}");
}

#[test]
fn project_settings_context_memory_saves_a_choice() {
  let mut page = context();
  assert_eq!(stored(&page).project_writes, WritePolicy::AfterApproval);
  page.choose("settings-memory-project-select", "Save automatically");
  page.until("project memory saving automatically", |p| {
    stored(p).project_writes == WritePolicy::Automatic
  });
  page.saved();
  assert_eq!(
    stored(&page).task_writes,
    WritePolicy::Automatic,
    "the other memory keeps its choice"
  );
}

#[test]
fn project_settings_context_memory_saves_a_limit_when_its_field_is_left() {
  let mut page = context();
  page.set("settings-memory-tokens-field", "4000");
  assert_eq!(page.text("project-settings-save-status"), "Changes not saved");
  focus(&mut page, "settings-memory-records-field");
  page.until("the new token limit", |p| stored(p).limits.tokens() == 4_000);
  page.saved();
  assert_eq!(stored(&page).limits.records(), 8, "the other limit is kept");
  assert_eq!(page.value("settings-memory-tokens-field"), "4,000");
  assert_eq!(page.text("settings-memory-tokens-field-read-back"), "= 4k tokens");
}

#[test]
fn project_settings_context_memory_refuses_an_mcp_clients_input() {
  let mut page = context();
  let before = stored(&page);
  with_synthetic_input(|| page.choose("settings-memory-project-select", "Save automatically"));
  page.until("the person-only note", |p| p.exists("settings-memory-person-only"));
  assert!(
    page
      .text("settings-memory-person-only")
      .contains("Only a person changes these settings")
  );
  assert_eq!(
    page.text("settings-memory-project-select"),
    "Save after approval",
    "the select is put back"
  );
  page.set("settings-memory-tokens-field", "20000");
  with_synthetic_input(|| focus(&mut page, "settings-memory-records-field"));
  assert_eq!(page.text("project-settings-save-status"), "Changes not saved");
  assert_eq!(
    stored(&page),
    before,
    "neither automatic saving nor a higher limit is stored"
  );
}

/// A refused choice puts its select back, and that is no choice: it never saves what a limit field holds, an MCP
/// client's typing here.
#[test]
fn project_settings_context_memory_refuses_a_limit_typed_before_a_refused_choice() {
  let mut page = context();
  let before = stored(&page);
  with_synthetic_input(|| page.set("settings-memory-tokens-field", "20000"));
  with_synthetic_input(|| page.choose("settings-memory-project-select", "Save automatically"));
  page.until("the person-only note", |p| p.exists("settings-memory-person-only"));
  page.until("the select put back", |p| {
    p.text("settings-memory-project-select") == "Save after approval"
  });
  for _ in 0..5 {
    page.window.pass();
  }
  assert!(page.exists("settings-memory-person-only"), "the note stays");
  assert_eq!(page.text("project-settings-save-status"), "Changes not saved");
  assert_eq!(stored(&page), before, "the typed limit is not stored");
  // A person's choice after it saves its policy alone; saves run one at a time, so a save of the limit would be in.
  page.choose("settings-memory-task-select", "Save after approval");
  page.until("task memory saving after approval", |p| {
    stored(p).task_writes == WritePolicy::AfterApproval
  });
  assert_eq!(
    stored(&page).limits,
    before.limits,
    "the typed limit is still not stored"
  );
}
