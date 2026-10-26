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
  assert!(!screen.text("agent-draft-bar").contains("→ CLI default"));
  screen.click("agent-effort");
  assert!(
    screen
      .text("agent-effort-menu")
      .contains("Uses the connection’s own setting")
  );
  assert!(screen.text("agent-effort-menu").contains("High"));
  screen.click("agent-effort-option-0");
  screen.set("agent-custom-model", "custom-test");
  screen.until("custom model saved on its own", |s| {
    s.listed().profile(id).expect("profile").profile.model().as_str() == "custom-test"
  });
  screen.until("the bar names the change", |s| {
    s.text("agent-draft-bar").contains("test-model → custom-test")
  });
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

#[test]
fn agents_on_an_api_provider_offer_its_models_and_take_any_effort_its_cli_does() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  let mut db = ProjectDb::open(&screen.root).expect("db");
  let stored = db.connections().expect("connections").remove(0);
  let mut connection = AgentConnection::try_from(stored).expect("CLI");
  connection
    .set_api_provider(Some(
      orchester_core::connection::ApiProvider::new(
        orchester_core::connection::BaseUrl::new("https://api.deepseek.com").expect("valid"),
        orchester_core::id::SecretId::generate(),
        vec![ModelId::new("deepseek-v4-pro").expect("model")],
      )
      .expect("a model"),
    ))
    .expect("on Default");
  db.save_connection(&connection).expect("on the provider");
  drop(db);
  screen.open(id);
  screen.set("agent-effort", "very high");
  assert!(
    screen.text("agent-model-section").contains("one word"),
    "Codex takes one word"
  );
  screen.set("agent-effort", "ultra");
  assert!(
    !screen.text("agent-model-section").contains("one word"),
    "any word the model advertises"
  );
  screen.click("agent-model");
  assert!(screen.text("agent-model-menu").contains("deepseek-v4-pro"));
}
