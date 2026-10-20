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
  fill_deepseek(&mut seeded);
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
  assert!(
    seeded
      .text("connection-plan-limits")
      .contains("Not read on an API provider")
  );
  assert!(
    !seeded.exists("connection-refresh-models"),
    "its models are typed, not listed"
  );
  assert!(
    seeded
      .text("connection-inspector-footer")
      .contains("with the key from SENTRY_TOKEN")
  );

  seeded.go(ProjectPage::EditConnection(connection.id()));
  seeded.until("the edit page with the provider", |seeded| {
    seeded.exists("connection-runs-sign-in") && seeded.exists("connection-provider-url")
  });
  seeded.click("connection-runs-sign-in");
  assert!(
    seeded
      .text("connection-off-provider")
      .contains("Saving takes Claude · Kontur off the API provider")
  );
  assert!(
    seeded
      .text("connection-off-provider")
      .contains("SENTRY_TOKEN stays in Secrets")
  );
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

#[test]
fn connections_api_provider_whose_key_was_archived_says_so_where_it_is_chosen() {
  let mut seeded = Seeded::new("connections");
  let sentry = seeded
    .db()
    .secrets()
    .expect("secrets read")
    .into_iter()
    .find(|secret| secret.name().as_str() == "SENTRY_TOKEN")
    .expect("the seeded secret");
  let mut connection = claude(&seeded);
  connection
    .set_api_provider(Some(
      orchester_core::connection::ApiProvider::new(
        orchester_core::connection::BaseUrl::new("https://api.deepseek.com/anthropic").expect("valid"),
        sentry.id(),
        vec![orchester_core::resource::ModelId::new("deepseek-flash").expect("valid")],
      )
      .expect("a model"),
    ))
    .expect("on Default");
  let mut db = seeded.db();
  db.save_connection(&connection.clone().into_connection())
    .expect("saved");
  drop(db);
  let keychain = super::support::signed_in::keychain();
  orchester_workspace::secrets::data::archive_in(&seeded.repo, keychain.get(), sentry.id()).expect("archived");
  seeded.list(orchester_app::routes::ConnectionsTab::AgentProviders);
  seeded.click(&Seeded::row(connection.id()));
  seeded.until("the inspector", |seeded| seeded.exists("connection-provider-key"));
  assert!(seeded.text("connection-provider-key").contains("Key secret missing"));
  assert!(seeded.text("connection-inspector-footer").contains("archived or gone"));
  seeded.go(ProjectPage::EditConnection(connection.id()));
  seeded.until("the edit page", |seeded| seeded.exists("connection-provider-key-error"));
  assert!(
    seeded
      .text("connection-provider-key-error")
      .contains("SENTRY_TOKEN was archived")
  );
  assert!(
    seeded
      .text("connection-form-reason")
      .contains("secret holding the API key")
  );
}

#[test]
fn connections_api_provider_rows_name_the_cli_its_version_and_the_providers_host() {
  let mut seeded = Seeded::new("api-providers");
  seeded.list(orchester_app::routes::ConnectionsTab::AgentProviders);
  let row = |seeded: &mut Seeded, named: &str| {
    let connection = seeded
      .db()
      .connections()
      .expect("connections read")
      .into_iter()
      .find(|connection| connection.name().as_str() == named)
      .expect("a seeded connection");
    seeded.text(&Seeded::row(connection.id()))
  };
  let deepseek = row(&mut seeded, "Claude · DeepSeek");
  assert!(
    deepseek.contains("Claude Code CLI 2.1.89 · api.deepseek.com"),
    "{deepseek}"
  );
  assert!(deepseek.contains("Connected"), "{deepseek}");
  let gateway = row(&mut seeded, "Claude · Gateway");
  assert!(gateway.contains("Claude Code CLI · localhost:4000"), "{gateway}");
  assert!(gateway.contains("Not checked"), "{gateway}");
  let old = row(&mut seeded, "Claude · Old gateway");
  assert!(old.contains("Key missing"), "{old}");
  for line in [&deepseek, &gateway, &old] {
    assert!(!line.contains("account"), "a key runs it, on no account: {line}");
  }
}

/// Edit connection's *Runs on* filled for DeepSeek's Anthropic API with SENTRY_TOKEN as the key, each field's reason
/// read on the way.
fn fill_deepseek(seeded: &mut Seeded) {
  seeded.click("connection-runs-provider");
  seeded.set("connection-provider-url", "https://api.deepseek.com/anthropic");
  assert!(
    seeded
      .text("connection-form-reason")
      .contains("secret holding the API key")
  );
  choose(seeded, "connection-provider-key", "SENTRY_TOKEN (Development)");
  assert!(seeded.text("connection-form-reason").contains("at least one model"));
  seeded.set("connection-provider-model", "deep seek");
  seeded.click("connection-provider-add-model");
  assert!(
    seeded
      .text("connection-provider-model-error")
      .contains("is not a model id")
  );
  // Only the current problem shows: clearing the draft, or typing another, takes the old one away (`UW2bO`).
  seeded.set("connection-provider-model", "");
  assert!(!seeded.exists("connection-provider-model-error"));
  seeded.set("connection-provider-model", "deep seek");
  seeded.click("connection-provider-add-model");
  assert!(seeded.exists("connection-provider-model-error"));
  seeded.set("connection-provider-model", "deep");
  assert!(!seeded.exists("connection-provider-model-error"));
  seeded.set("connection-provider-model", "deepseek-flash, deepseek-v4-pro");
  seeded.click("connection-provider-add-model");
  assert!(!seeded.exists("connection-provider-model-error"));
  assert!(seeded.exists("connection-provider-model-1"), "each id is a chip");
}

/// The row of `menu`'s options whose first words are `label`, with all its words.
fn menu_row(seeded: &mut Seeded, menu: &str, label: &str) -> (String, String) {
  for index in 0.. {
    let option = format!("{menu}-option-{index}");
    let Some(texts) = seeded.window.texts(&option) else {
      break;
    };
    if texts.first().map(String::as_str) == Some(label) {
      return (option, texts.join(" "));
    }
  }
  panic!("{menu} lists {label}")
}

#[test]
fn connections_api_provider_is_listed_unavailable_with_its_whole_reason_for_the_orchestrator_and_the_reviewer() {
  let mut seeded = Seeded::new("api-providers");
  seeded.go(ProjectPage::OrchestratorSettings);
  seeded.until("the Orchestrator's settings", |seeded| {
    seeded.exists("orchestrator-connection")
  });
  seeded.click("orchestrator-connection");
  let (option, words) = menu_row(&mut seeded, "orchestrator-connection", "Claude · DeepSeek");
  assert!(seeded.exists(&format!("{option}-icon")), "the agent glyph leads it");
  assert!(
    words.ends_with("This connection runs on an API provider; choose another one."),
    "the whole reason, not cut: {words}"
  );

  seeded.go(ProjectPage::Settings(Default::default()));
  seeded.until("the reviewer", |seeded| seeded.exists("settings-reviewer-connection"));
  seeded.click("settings-reviewer-connection");
  let (option, words) = menu_row(&mut seeded, "settings-reviewer-connection", "Codex · DeepSeek");
  assert!(seeded.exists(&format!("{option}-icon")), "the agent glyph leads it");
  assert!(
    words.contains("Auto mode’s reviewer runs on a CLI’s own sign-in") && words.ends_with("choose another one."),
    "the whole reason: {words}"
  );
  let before = seeded.db().auto_mode_settings().expect("settings");
  seeded.click(&option);
  assert_eq!(
    seeded.db().auto_mode_settings().expect("settings"),
    before,
    "an unavailable connection is not chosen"
  );
}
