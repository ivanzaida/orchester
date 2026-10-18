//! An agent CLI connection on an API provider (`docs/design/api-providers.md`), in the window: *Runs on* takes the
//! place of the account, says which API each CLI needs, and checks the base URL, key and models; Edit connection runs
//! the seeded Claude Code connection on a provider with an existing key secret, shows it in the inspector, and runs
//! it on its own sign-in again. No key value is typed: the window's secret store is the computer's, so the tests only
//! pick a secret the project has.

use orchester_app::routes::{NewConnection, ProjectPage};
use orchester_core::{
  connection::{AgentConnection, ConnectionStatus},
  store::{ConnectionStore, SecretStore},
};

use super::harness::Seeded;

/// The seeded Claude Code connection.
fn claude(seeded: &Seeded) -> AgentConnection {
  let stored = seeded
    .db()
    .connections()
    .expect("connections read")
    .into_iter()
    .find(|connection| connection.name().as_str() == "Claude · Kontur")
    .expect("the seeded Claude connection");
  AgentConnection::try_from(stored).expect("an agent connection")
}

#[test]
fn connections_api_provider_takes_the_accounts_place_and_names_each_clis_api() {
  let mut seeded = Seeded::new("connections");
  seeded.add(NewConnection::AgentProvider);
  seeded.click("connection-kind-claude");
  assert!(
    seeded.exists("connection-account"),
    "on its sign-in the CLI runs on an account"
  );
  assert!(!seeded.exists("connection-provider-url"));
  seeded.click("connection-runs-provider");
  assert!(!seeded.exists("connection-account"), "a key runs it, on no account");
  assert!(
    seeded
      .text("connection-provider-section")
      .contains("Anthropic Messages API")
  );
  assert!(!seeded.exists("connection-auth"), "no sign-in notice for a key");
  seeded.click("connection-kind-codex");
  assert!(
    seeded
      .text("connection-provider-section")
      .contains("OpenAI Responses API")
  );
  seeded.set("connection-provider-url", "http://api.deepseek.com");
  assert!(
    seeded
      .text("connection-provider-section")
      .contains("starts with https://")
  );
  seeded.set("connection-provider-url", "https://api.deepseek.com");
  assert!(seeded.text("connection-provider-notice").contains("api.deepseek.com"));
  seeded.click("connection-runs-sign-in");
  assert!(seeded.exists("connection-account"));
  assert!(!seeded.exists("connection-provider-url"));
}

#[test]
fn connections_api_provider_is_chosen_in_edit_connection_and_shown_in_the_inspector() {
  let mut seeded = Seeded::new("connections");
  let connection = claude(&seeded);
  let sentry = seeded
    .db()
    .secrets()
    .expect("secrets read")
    .into_iter()
    .find(|secret| secret.name().as_str() == "SENTRY_TOKEN")
    .expect("the seeded secret");
  seeded.go(ProjectPage::EditConnection(connection.id()));
  seeded.until("the edit page", |seeded| seeded.exists("connection-runs-provider"));
  seeded.click("connection-runs-provider");
  seeded.set("connection-provider-url", "https://api.deepseek.com/anthropic");
  assert!(
    seeded
      .text("connection-form-reason")
      .contains("secret holding the API key")
  );
  choose(&mut seeded, "connection-provider-key", "SENTRY_TOKEN (development)");
  assert!(seeded.text("connection-form-reason").contains("at least one model"));
  seeded.set("connection-provider-models", "deepseek-flash, deepseek-v4-pro");
  assert!(!seeded.exists("connection-form-reason"), "nothing keeps it from saving");
  lurq::app::events::with_synthetic_input(|| seeded.click("connection-edit-save"));
  assert!(
    seeded.text("connection-form-reason").contains("Only a person"),
    "an MCP client's click is refused, and the page says so"
  );
  assert!(claude(&seeded).api_provider().is_none(), "nothing was saved");
  seeded.click("connection-edit-save");
  seeded.until("the inspector", |seeded| seeded.exists("connection-provider-base-url"));

  let saved = claude(&seeded);
  let provider = saved.api_provider().expect("it runs on the provider");
  assert_eq!(provider.base_url().as_str(), "https://api.deepseek.com/anthropic");
  assert_eq!(provider.key(), sentry.id());
  let models: Vec<&str> = provider.model_ids().map(|model| model.as_str()).collect();
  assert_eq!(models, ["deepseek-flash", "deepseek-v4-pro"]);
  assert_eq!(
    saved.status_with(std::slice::from_ref(&sentry)),
    ConnectionStatus::Connected,
    "its CLI's own sign-in is not used"
  );
  assert_eq!(
    seeded.text("connection-provider-base-url"),
    "Base URL https://api.deepseek.com/anthropic"
  );
  assert!(seeded.text("connection-provider-key").contains("SENTRY_TOKEN"));
  assert!(!seeded.exists("connection-account-link"), "no account on a key");

  seeded.go(ProjectPage::EditConnection(connection.id()));
  seeded.until("the edit page with the provider", |seeded| {
    seeded.exists("connection-runs-sign-in") && seeded.exists("connection-provider-url")
  });
  seeded.click("connection-runs-sign-in");
  seeded.click("connection-edit-save");
  seeded.until("the inspector", |seeded| seeded.exists("connection-inspector"));
  seeded.until("the connection runs on its sign-in", |seeded| {
    claude(seeded).api_provider().is_none()
  });
}

/// Chooses the option labelled `label` of select `id`.
fn choose(seeded: &mut Seeded, id: &str, label: &str) {
  let mut select = seeded
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .unwrap_or_else(|| panic!("{id} is a select"));
  let index = select
    .labels()
    .iter()
    .position(|value| value.as_ref() == label)
    .unwrap_or_else(|| panic!("{label} among {:?}", select.labels()));
  select.commit(index);
  seeded.window.pass();
}
