//! What the Orchestrator runs on, from the composer's chip (`05 Account and model picker` `v6mWp8`, `05b On an API
//! provider` `AZp3I`): the picker lists the project's sign-in connections by CLI and its API-provider connections with
//! their tokens and no plan limit; choosing one publishes the Orchestrator's next version as its settings page does,
//! only once the person presses *Publish vN*.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{orchestrator::OrchestratorSettings, resource::ModelId, store::ConnectionStore};

use super::{
  connections,
  harness::{Orchestrator, Setup},
};

#[test]
fn orchestrator_picker_lists_sign_ins_and_api_providers_and_publishes_the_choice() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let program = std::env::current_exe().expect("a program");
  let deepseek = connections::api_provider("DeepSeek", &program);
  screen.db().create_connection(&deepseek).expect("the API provider's connection");
  screen.go(OrchestratorChat::New);
  screen.click("orchestrator-runs-on-chip");
  screen.until("the picker", |screen| screen.exists("orchestrator-runs-on-menu"));
  let menu = screen.text("orchestrator-runs-on-menu");
  for part in [
    "Claude Code · your sign-ins",
    "Codex · your sign-ins",
    "API providers · pay per token",
    "Claude Code · api.deepseek.com",
    "0 today",
    "no plan limit",
    "Another account starts a new session: earlier messages aren't remembered.",
    "Settings ↗",
  ] {
    assert!(menu.contains(part), "{part} in {menu}");
  }
  assert_eq!(
    screen.text(&format!("orchestrator-runs-on-{}-name", deepseek.id())),
    "DeepSeek"
  );
  // What it runs on now is chosen; the effort row offers the connection's efforts, Default first.
  assert_eq!(screen.text("orchestrator-runs-on-model-value"), "haiku");
  assert!(screen.exists("orchestrator-runs-on-effort"));
  assert!(!screen.exists("orchestrator-runs-on-publish"), "nothing changed yet");

  screen.press(&format!("orchestrator-runs-on-{}", deepseek.id()));
  screen.until("the provider chosen", |screen| screen.exists("orchestrator-runs-on-sent-to"));
  assert!(screen.text("orchestrator-runs-on-sent-to").contains("api.deepseek.com"));
  assert_eq!(screen.text("orchestrator-runs-on-model-value"), "deepseek-v4-pro");
  assert!(
    !screen.exists("orchestrator-runs-on-effort"),
    "a provider that lists no efforts has no effort row"
  );
  assert_eq!(
    screen.db().orchestrator_published().expect("read").map(|(version, _)| version),
    Some(1),
    "choosing alone changes nothing"
  );
  assert_eq!(screen.text("orchestrator-runs-on-publish"), "Publish v2");
  screen.click("orchestrator-runs-on-publish");
  let chosen = OrchestratorSettings::new(
    deepseek.id(),
    ModelId::new("deepseek-v4-pro").expect("model"),
    None,
  );
  screen.until("the next version", |screen| {
    screen.db().orchestrator_published().expect("read") == Some((2, chosen.clone()))
  });
  screen.until("the chip", |screen| {
    screen.text("orchestrator-runs-on") == "DeepSeek · deepseek-v4-pro"
  });
}
