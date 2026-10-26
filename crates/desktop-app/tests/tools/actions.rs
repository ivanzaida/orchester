//! A command tool saves its draft on its own; a widening change asks at its field first, and publishing asks again
//! only for what was not allowed on the page. Discard draft and Remove are asked in place; the page's tabs show what
//! the command takes and gives.
use orchester_app::routes::{ProjectPage, Route, ToolTab};
use orchester_core::{resource::Tool, store::ResourceStore};
use orchester_workspace::{agents::data::VersionState, tools::data::ProjectTools};

use super::harness::{Tools, command_text};

#[test]
fn tools_new_draft_saves_all_fields_then_publishes_with_questions() {
  let mut t = Tools::new();
  t.go(ProjectPage::Tools);
  assert!(t.exists("tools-empty"));
  t.click("tools-create-command");
  t.window
    .until("new form", |w| w.tree.get_element_by_id("tool-name").is_some());
  t.set("tool-name", "Run tests");
  t.set(
    "tool-program",
    &std::env::current_exe().expect("test binary").to_string_lossy(),
  );
  t.set("tool-description", "Checks the change.");
  t.set("tool-arguments", "test\n--workspace");
  t.set("tool-timeout", "90");
  t.set("tool-exitCodes", "0, 3");
  t.set("tool-outputLimit", "64");
  t.set("tool-environment", "CI=true\nRUST_LOG=info");
  t.ready();
  // Create draft is pressable once the program's check has passed.
  t.window.until("checked", |w| {
    w.tree.tick_futures();
    w.pass();
    if let Some(mut create) = w.tree.get_element_by_id_mut("tool-create") {
      create.click();
    }
    w.pass();
    w.tree.get_element_by_id("tool-confirm").is_some()
  });
  assert!(t.exists("tool-confirm"));
  assert!(
    t.text("tool-confirm")
      .contains("your rights, network access and no sandbox")
  );
  t.confirm();
  t.wait_state("Draft");
  let Route::Project(_, ProjectPage::Tool(id, _)) = t.window.navigation().route_now() else {
    panic!("saved editor route")
  };
  let text = command_text(&t.entry(id).tool);
  assert_eq!(text.description, "Checks the change.");
  assert_eq!(text.arguments, "test\n--workspace");
  assert_eq!(text.timeout, "90");
  assert_eq!(text.exit_codes, "0, 3");
  assert_eq!(text.output_limit, "64");
  assert_eq!(text.environment, "CI=true\nRUST_LOG=info");
  assert!(std::path::Path::new(&text.program).is_absolute());
  t.wait_for("tool-publish");
  t.click("tool-publish");
  t.confirm();
  t.wait_state("Published v1");
  assert!(matches!(t.entry(id).state, VersionState::Published(_)));
  assert!(t.db().draft::<Tool>(id).expect("draft read").is_none());
}

#[test]
fn tools_narrowing_saves_on_its_own_and_widening_asks_at_its_field() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  t.edit(id);
  assert!(!t.exists("tool-save"), "no Save draft button");
  t.set("tool-timeout", "60");
  t.wait_state("Draft");
  assert!(!t.exists("tool-widen"), "narrowing asks nothing");
  assert_eq!(command_text(&t.entry(id).tool).timeout, "60");
  t.click("tool-publish");
  assert!(!t.exists("tool-confirm"));
  t.wait_state("Published v2");
  t.set("tool-arguments", "test\n--all-features");
  t.wait_for("tool-widen");
  t.click("tool-widen-keep");
  assert!(!t.exists("tool-widen"));
  assert_eq!(command_text(&t.entry(id).tool).arguments, "test");
  t.set("tool-arguments", "test\n--all-targets");
  t.widen();
  t.wait_state("Draft");
  assert_eq!(command_text(&t.entry(id).tool).arguments, "test\n--all-targets");
  t.click("tool-publish");
  assert!(!t.exists("tool-confirm"), "allowed at its field already");
  t.wait_state("Published v3");
}

#[test]
fn tools_discard_draft_returns_to_the_published_version() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  t.edit(id);
  t.set("tool-timeout", "60");
  t.wait_state("Draft");
  t.wait_for("tool-discard");
  t.click("tool-discard");
  t.wait_for("hub-discard-confirm");
  t.click("hub-discard-confirm");
  t.window.until("discarded", |w| {
    w.tree.tick_futures();
    w.pass();
    w.texts("tool-draft-bar-status").is_some_and(|s| s.join(" ").contains("Published v1"))
  });
  assert!(t.db().draft::<Tool>(id).expect("draft read").is_none());
  assert_eq!(command_text(&t.entry(id).tool).timeout, command_text(&super::harness::command()).timeout);
}

#[test]
fn tools_remove_an_unused_tool() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  t.edit(id);
  t.click("tool-more");
  t.click("tool-remove");
  t.wait_for("hub-remove-confirm");
  assert!(t.text("hub-remove").contains("No pipeline step uses it"));
  // Remove is pressable once the question has read who uses the tool.
  t.window.until("the list", |w| {
    w.tree.tick_futures();
    w.pass();
    if let Some(mut confirm) = w.tree.get_element_by_id_mut("hub-remove-confirm") {
      confirm.click();
    }
    w.pass();
    w.tree.get_element_by_id("tools-library").is_some()
  });
  assert!(ProjectTools::load(&t.db()).expect("tools").tool(id).is_none());
}

#[test]
fn tools_page_tabs_show_inputs_outputs_access_and_usage() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  t.edit(id);
  for tab in ["configuration", "inputs_outputs", "access", "usage_log"] {
    assert!(t.exists(&format!("tool-tab-{tab}")), "{tab}");
  }
  t.click("tool-tab-inputs_outputs");
  t.wait_for("tool-inputs-outputs");
  assert_eq!(
    t.window.navigation().route_now(),
    Route::Project(super::support::project_id(), ProjectPage::Tool(id, ToolTab::InputsOutputs))
  );
  let shown = t.text("tool-inputs-outputs");
  assert!(shown.contains("No inputs") && shown.contains("Exit code") && shown.contains("Output"), "{shown}");
  t.click("tool-tab-usage_log");
  t.wait_for("tool-usage-unavailable");
  t.click("tool-tab-configuration");
  t.wait_for("tool-fields");
}
