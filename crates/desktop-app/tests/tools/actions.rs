use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{resource::Tool, store::ResourceStore};
use orchester_workspace::agents::data::VersionState;

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
  t.click("tool-save");
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
  t.click("tool-publish");
  assert!(t.exists("tool-confirm"));
  t.confirm();
  t.wait_state("Published v1");
  t.ready();
  assert!(matches!(t.entry(id).state, VersionState::Published(_)));
  assert!(t.db().draft::<Tool>(id).expect("draft read").is_none());
  assert!(t.text("tool-action-reason").contains("No changes since v1"));
}

#[test]
fn tools_narrowing_saves_without_question_and_widening_asks_twice() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  t.edit(id);
  t.set("tool-timeout", "60");
  t.ready();
  t.click("tool-save");
  assert!(!t.exists("tool-confirm"));
  t.wait_state("Draft");
  t.click("tool-publish");
  assert!(!t.exists("tool-confirm"));
  t.wait_state("Published v2");
  t.set("tool-arguments", "test\n--all-features");
  t.ready();
  t.click("tool-save");
  assert!(t.exists("tool-confirm"));
  t.click("tool-confirm-keep");
  assert!(!t.exists("tool-confirm"));
  assert_eq!(command_text(&t.entry(id).tool).arguments, "test");
  t.click("tool-save");
  t.confirm();
  t.wait_state("Draft");
  t.click("tool-publish");
  assert!(t.exists("tool-confirm"));
  t.confirm();
  t.wait_state("Published v3");
}
