use orchester_core::{connection::AgentConnection, resource::ModelId, store::ConnectionStore};
use orchester_store::ProjectDb;

use super::harness::Agents;

#[test]
fn agents_custom_model_summary_matches_saved_value_and_effort_options() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.click("agent-model");
  assert!(screen.text("agent-model-menu").contains("Test Codex"));
  assert!(
    screen
      .text("agent-model-menu")
      .contains("Enter an id the connection accepts")
  );
  screen.click("agent-model-option-1");
  screen.set("agent-custom-model", "test-model");
  assert!(!screen.exists("agent-save"), "the resolved profile is unchanged");
  assert!(!screen.text("agent-draft-card").contains("→ CLI default"));
  screen.click("agent-effort");
  assert!(
    screen
      .text("agent-effort-menu")
      .contains("Uses the connection’s own setting")
  );
  assert!(screen.text("agent-effort-menu").contains("High"));
  screen.click("agent-effort-option-0");
  screen.set("agent-custom-model", "custom-test");
  assert!(screen.text("agent-save-status").contains("Unsaved changes"));
  assert!(screen.text("agent-draft-card").contains("test-model → custom-test"));
  screen.click("agent-save");
  screen.until("custom model saved", |s| {
    !s.exists("agent-save") && !s.exists("agent-saving")
  });
  assert_eq!(
    screen.listed().profile(id).expect("profile").profile.model().as_str(),
    "custom-test"
  );
}

#[test]
fn agents_custom_model_warning_opens_model_menu_and_connection() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  let mut db = ProjectDb::open(&screen.root).expect("db");
  let stored = db.connections().expect("connections").remove(0);
  let mut connection = AgentConnection::try_from(stored).expect("CLI");
  connection.record_model_requirement(&ModelId::new("custom-test").expect("model"), "9.0.0");
  db.save_connection(&connection).expect("requirement");
  screen.open(id);
  screen.click("agent-model");
  screen.click("agent-model-option-1");
  screen.set("agent-custom-model", "custom-test");
  assert!(screen.text("agent-model-too-old").contains("9.0.0"));
  screen.click("agent-choose-model");
  assert!(screen.exists("agent-model-option-0"));
  screen.click("agent-model-option-0");
  assert!(!screen.exists("agent-model-too-old"));
  screen.click("agent-model");
  screen.click("agent-model-option-1");
  screen.click("agent-open-connection");
  assert!(
    screen
      .window
      .navigation()
      .route_now()
      .path()
      .contains(&connection.id().to_string())
  );
}
