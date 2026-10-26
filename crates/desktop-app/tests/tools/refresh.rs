//! A transient read failure must not discard the editor's unsaved values.
use orchester_app::routes::ProjectPage;
use orchester_core::{resource::Draft, store::ResourceStore, time::Timestamp};

use super::harness::{Tools, command_text};

#[test]
fn tools_refresh_failure_and_retry_preserve_unsaved_fields() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  t.edit(id);
  let local = [
    ("tool-description", "Unsaved local description"),
    ("tool-arguments", "test\n--all-features"),
    ("tool-timeout", "not yet a number"),
    ("tool-environment", "CI=true\nRUST_LOG=info"),
  ];
  for (field, value) in local {
    t.set(field, value);
  }
  let schema = Schema::of(&t);
  schema.fail();
  refresh(&t);
  wait_failed(&mut t);
  assert_fields(&mut t, &local);
  t.click("tools-load-error-retry");
  wait_failed(&mut t);
  assert_fields(&mut t, &local);
  schema.restore();
  let mut stored = command_text(&t.entry(id).tool);
  stored.description = "Saved elsewhere".into();
  t.db()
    .save_draft(
      id,
      &Draft::new(
        stored.tool(None).expect("tool"),
        t.entry(id).state.published(),
        Timestamp::now(),
      ),
    )
    .expect("outside draft");
  t.click("tools-load-error-retry");
  t.window.until("successful retry reads the outside change", |w| {
    w.tree.get_element_by_id("tools-load-error").is_none() && w.tree.get_element_by_id("tool-changed").is_some()
  });
  assert_fields(&mut t, &local);
  assert_eq!(command_text(&t.entry(id).tool).description, "Saved elsewhere", "nothing saves over it");
  t.click("tool-reload");
  t.window.until("explicit reload replaces the local values", |w| {
    w.tree.get_element_by_id("tool-editor").is_some() && w.tree.get_element_by_id("tool-changed").is_none()
  });
  assert_fields(
    &mut t,
    &[("tool-description", "Saved elsewhere"), ("tool-timeout", "600")],
  );
}

#[test]
fn tools_new_editor_preserves_unsaved_values_through_failed_refresh_and_retry() {
  let mut t = Tools::new();
  t.go(ProjectPage::NewTool);
  let local = [("tool-name", "New checks"), ("tool-description", "Still editing")];
  for (field, value) in local {
    t.set(field, value);
  }
  let schema = Schema::of(&t);
  schema.fail();
  refresh(&t);
  wait_failed(&mut t);
  assert_fields(&mut t, &local);
  schema.restore();
  t.click("tools-load-error-retry");
  t.window.until("new editor refreshed", |w| {
    w.tree.get_element_by_id("tool-editor").is_some() && w.tree.get_element_by_id("tools-load-error").is_none()
  });
  assert_fields(&mut t, &local);
  assert!(
    orchester_workspace::tools::data::ProjectTools::load(&t.db())
      .expect("tools")
      .tools
      .is_empty()
  );
}

#[test]
fn tools_retry_revalidates_after_storage_recovers_without_changing_fields() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  t.edit(id);
  let schema = Schema::of(&t);
  schema.fail();
  t.set("tool-description", "Keep this unsaved description");
  t.window.until("validation fails during the outage", |w| {
    w.tree.get_element_by_id("tool-program-note").is_some()
  });
  refresh(&t);
  wait_failed(&mut t);
  schema.restore();
  t.click("tools-load-error-retry");
  t.window.until("successful retry also reruns validation", |w| {
    w.tree.get_element_by_id("tools-load-error").is_none() && w.tree.get_element_by_id("tool-program-note").is_none()
  });
  assert_fields(&mut t, &[("tool-description", "Keep this unsaved description")]);
  // Once the program checks out again, the draft saves on its own.
  t.wait_state("Draft saved");
  assert_eq!(
    command_text(&t.entry(id).tool).description,
    "Keep this unsaved description"
  );
}

fn refresh(t: &Tools) {
  t.window
    .projects()
    .feed(super::support::project_id())
    .outside
    .update(|c| c.tools += 1);
}
fn wait_failed(t: &mut Tools) {
  t.window.until("failed Tools refresh", |w| {
    w.tree.get_element_by_id("tools-load-error").is_some()
  });
}
fn assert_fields(t: &mut Tools, values: &[(&str, &str)]) {
  assert!(t.exists("tool-editor"), "a read failure keeps the editor mounted");
  for (id, expected) in values {
    let value = t
      .window
      .tree
      .get_element_by_id_mut(id)
      .expect("field")
      .as_text_input()
      .expect("text input")
      .value();
    assert_eq!(&value, expected, "{id} keeps its unsaved value");
  }
}

struct Schema {
  connection: rusqlite::Connection,
  version: u32,
}
impl Schema {
  fn of(t: &Tools) -> Self {
    let connection = rusqlite::Connection::open(t.repo.join(".orchester/orchester.db")).expect("db");
    let version = connection
      .query_row("PRAGMA user_version", [], |r| r.get(0))
      .expect("schema version");
    Self { connection, version }
  }
  fn fail(&self) {
    self
      .connection
      .execute_batch("PRAGMA user_version = 99999")
      .expect("future schema");
  }
  fn restore(&self) {
    self
      .connection
      .execute_batch(&format!("PRAGMA user_version = {}", self.version))
      .expect("restore schema");
  }
}
