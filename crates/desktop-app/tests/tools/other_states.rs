//! Library errors, unsupported kinds and refreshing a changed editor.
use orchester_app::routes::{ProjectPage, ToolTab};
use orchester_core::{
  id::SecretId,
  name::Name,
  resource::{AccessPolicy, AccessWindow, ApprovalMode, DatabaseQuery, DeniedBehavior, Draft, Tool, ToolAction},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_workspace::tools::data::create_tool;

use super::harness::Tools;

#[test]
fn tools_unsupported_kind_is_listed_and_cannot_be_edited() {
  let mut t = Tools::new();
  let policy = AccessPolicy::new(
    Default::default(),
    Name::new("dev").expect("name"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("policy");
  let tool = Tool::new(
    "Reads the database",
    ToolAction::DatabaseQuery(DatabaseQuery::new(SecretId::generate(), "select 1".into()).expect("query")),
    Some(policy),
  )
  .expect("tool");
  let id = create_tool(
    &mut t.db(),
    &Name::new("Database query").expect("name"),
    &tool,
    Timestamp::now(),
  )
  .expect("created");
  t.go(ProjectPage::Tools);
  let row = t.text(&format!("tool-row-{id}"));
  assert!(row.contains("Database") && row.contains("Unavailable"));
  t.edit(id);
  assert!(t.exists("tool-read-only"));
  assert!(!t.exists("tool-program"));
  t.click("tool-publish");
  assert!(!t.exists("tool-confirm"));
  assert_eq!(t.entry(id).tool, tool);
}

#[test]
fn tools_load_error_is_explicit_and_retry_reads_current_data() {
  let mut t = Tools::new();
  t.go(ProjectPage::Tools);
  let sqlite = rusqlite::Connection::open(t.repo.join(".orchester/orchester.db")).expect("db");
  let version: u32 = sqlite
    .query_row("PRAGMA user_version", [], |r| r.get(0))
    .expect("schema version");
  sqlite
    .execute_batch("PRAGMA user_version = 99999")
    .expect("future schema");
  t.window
    .projects()
    .feed(super::support::project_id())
    .outside
    .update(|c| c.tools += 1);
  t.window.until("library read error", |w| {
    w.tree.get_element_by_id("tools-load-error").is_some()
  });
  assert!(
    t.exists("tools-add"),
    "the library header remains available during a read failure"
  );
  assert!(t.text("tools-library").contains("Couldn’t load the project’s tools"));
  assert!(!t.exists("tools-empty"), "a failed read never claims an empty project");
  // The fixture's database returns to its original schema; Retry must read it again.
  sqlite
    .execute_batch(&format!("PRAGMA user_version = {version}"))
    .expect("restore schema");
  t.click("tools-load-error-retry");
  t.window.until("read after retry", |w| {
    w.tree.get_element_by_id("tools-empty").is_some()
  });
}
#[test]
fn tools_external_change_preserves_unsaved_fields_and_requires_reload() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  t.edit(id);
  t.set("tool-description", "Unsaved local change");
  let mut text = super::harness::command_text(&t.entry(id).tool);
  text.description = "Saved elsewhere".into();
  t.db()
    .save_draft(
      id,
      &Draft::new(
        text.tool(None).expect("tool"),
        t.entry(id).state.published(),
        Timestamp::now(),
      ),
    )
    .expect("draft");
  t.window
    .projects()
    .feed(super::support::project_id())
    .outside
    .update(|c| c.tools += 1);
  t.window.until("changed tool notice", |w| {
    w.tree.get_element_by_id("tool-changed").is_some()
  });
  assert!(!t.exists("tool-confirm") && !t.exists("tool-widen"));
  let value = t
    .window
    .tree
    .get_element_by_id_mut("tool-description")
    .expect("input")
    .as_text_input()
    .expect("text")
    .value();
  assert_eq!(value, "Unsaved local change");
  t.click("tool-reload");
  t.window.until("reloaded editor", |w| {
    w.tree.get_element_by_id("tool-editor").is_some() && w.tree.get_element_by_id("tool-changed").is_none()
  });
  assert_eq!(
    super::harness::command_text(&t.entry(id).tool).description,
    "Saved elsewhere"
  );
  t.go(ProjectPage::Tool(id, ToolTab::InputsOutputs));
  assert!(t.exists("tool-inputs-outputs"));
}
