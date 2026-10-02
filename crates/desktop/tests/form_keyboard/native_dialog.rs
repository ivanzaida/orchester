//! Adding and editing a native connection (04.H1 `Byo8K`, H1B `h2OUr`, H3 `KGciR`) and its models
//! (04.H6 `zHowy`): the four kinds, the Official preset with the others unavailable, the key secret
//! picked or created with the connection, models added by hand with their default model and effort,
//! provider fallback, why the form cannot save, and the models dialog. Key values here are fake, and
//! the keychain is the harness's one in memory.

use std::path::Path;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CheckResult, ConnectionStatus, Effort, EndpointPreset, ModelSource, NativeModels,
  },
  id::ConnectionId,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::ModelId,
  secret::{Environment, SecretKind},
  store::{ConnectionStore, SecretStore},
};
use orchester_store::ProjectDb;

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
  native_connections::{NativeSeed, key_secret, model, native_connection, open_connections, profile},
  task_run::has,
};

const DIALOG: &str = "connection-dialog";
const REASON: &str = "connection-save-reason";
const KEY_VALUE: &str = "fake-key-Q7x2";

pub(crate) fn add_native(desktop: &mut Desktop) {
  desktop.wait_for("nav-connections");
  desktop.click("nav-connections");
  desktop.wait_for("add-connection");
  desktop.click("add-connection");
  desktop.wait_for(DIALOG);
  desktop.press("connection-kind-anthropic");
  desktop.wait_for("native-preset");
}

pub(crate) fn set(desktop: &mut Desktop, id: &str, text: &str) {
  desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .unwrap_or_else(|| panic!("text input {id}"))
    .set_value(text);
  desktop.pass();
}

/// Chooses the option of the select `id` whose label contains `label`.
pub(crate) fn choose(desktop: &mut Desktop, id: &str, label: &str) {
  let mut select = desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .unwrap_or_else(|| panic!("the select {id}"));
  let index = select
    .labels()
    .iter()
    .position(|option| option.contains(label))
    .unwrap_or_else(|| panic!("{id} offers {label}"));
  select.commit(index);
  desktop.pass();
}

/// Whether the action bar says why the connection cannot be saved.
fn has_reason(desktop: &mut Desktop) -> bool {
  desktop.tree.get_element_by_id(REASON).is_some()
}

fn add_model(desktop: &mut Desktop, id: &str, limits: (&str, &str), efforts: &[&str]) {
  desktop.click("model-add");
  desktop.wait_for("model-editor");
  set(desktop, "model-id", id);
  set(desktop, "model-context", limits.0);
  set(desktop, "model-max-output", limits.1);
  for effort in efforts {
    desktop.click(&format!("model-effort-{effort}"));
  }
  desktop.click("model-save");
  desktop.wait_for(&format!("model-row-{id}"));
}

fn whole_screen_shows(desktop: &mut Desktop, text: &str) -> bool {
  desktop.tree.root().is_some_and(|root| contains_text(root, text))
}

#[test]
fn add_connection_offers_four_kinds_and_the_official_anthropic_preset() {
  let mut desktop = Desktop::with_project_named("Kontur");
  desktop.wait_for("nav-connections");
  desktop.click("nav-connections");
  desktop.wait_for("add-connection");
  desktop.click("add-connection");
  desktop.wait_for(DIALOG);
  for (kind, title) in [
    ("connection-kind-codex", "Codex CLI"),
    ("connection-kind-claude", "Claude Code CLI"),
    ("connection-kind-anthropic", "Anthropic API (native)"),
    ("connection-kind-openai", "OpenAI API (native)"),
  ] {
    assert!(shows(&mut desktop, kind, title), "{title}");
  }
  assert!(whole_screen_shows(
    &mut desktop,
    "OpenAI API connections come in a later version of Orchester."
  ));
  // The OpenAI card cannot be chosen: pressing it changes nothing.
  desktop.press("connection-kind-openai");
  assert!(!has(&mut desktop, "native-preset"));
  desktop.press("connection-kind-anthropic");
  desktop.wait_for("native-preset");
  assert_eq!(desktop.value("connection-name"), "Anthropic API · Kontur");
  for text in [
    "Official",
    "api.anthropic.com",
    "DeepSeek",
    "api.deepseek.com",
    "The DeepSeek preset uses OpenAI Responses; DeepSeek’s Anthropic route is reachable through Custom,",
    "and compatible endpoints come in a later version of Orchester.",
    "Custom",
    "your server",
  ] {
    assert!(shows(&mut desktop, "native-preset", text), "{text}");
  }
  assert!(shows(&mut desktop, "native-url", "https://api.anthropic.com"));
  assert!(shows(&mut desktop, "native-test", "Not checked"));
  assert!(shows(
    &mut desktop,
    "native-test",
    "Testing a native connection comes with native agent runs in a later version."
  ));
  for text in [
    "API key · project secret",
    "Models",
    "Fetching models needs a test of the connection",
    "Add a model by hand",
    "Default model",
    "Default effort",
    "Provider fallback",
    "Retry declined requests on another model",
    "Native kinds need no CLI",
    "Set by the preset. Prompts, repository content and command output are sent to api.anthropic.com.",
    "The connection keeps only which secret it uses.",
    "Used by agent profiles that do not choose a model.",
    "After saving, choose this connection and a model in an agent profile.",
  ] {
    assert!(shows(&mut desktop, DIALOG, text), "{text}");
  }
  assert!(shows(&mut desktop, "native-fallback", "Retry declined requests"));
  assert!(shows(&mut desktop, REASON, "Choose or create the API key’s secret."));
  // Choosing DeepSeek or Custom does nothing.
  desktop.press("native-preset-custom");
  desktop.press("native-preset-deepseek");
  assert!(shows(&mut desktop, "native-url", "https://api.anthropic.com"));
}

/// Tab goes through the native form's controls in the order they read and stays in the dialog;
/// what cannot be used (the OpenAI kind, DeepSeek and Custom, Test again, Fetch models) is no stop.
#[test]
fn native_dialog_tab_reaches_its_controls_in_order_and_stays_in_the_dialog() {
  let mut desktop = Desktop::with_project_named("Kontur");
  add_native(&mut desktop);
  // The type select has no id of its own and shows up as an unnamed stop.
  let stops: Vec<String> = desktop.stops(40, false).into_iter().flatten().collect();
  let order = [
    "connection-dialog-close",
    "connection-name",
    "connection-kind-codex",
    "connection-kind-claude",
    "connection-kind-anthropic",
    "native-preset-official",
    "native-url-advanced",
    "native-key-select",
    "native-key-new",
    "model-add",
    "native-default-model-select",
    "native-default-effort-select",
    "native-fallback",
    "connection-cancel",
  ];
  assert_eq!(&stops[..order.len()], order, "the first round of stops");
  for stop in &stops {
    assert!(order.contains(&stop.as_str()), "Tab left the dialog for {stop}");
  }
  for never in [
    "connection-kind-openai",
    "native-preset-deepseek",
    "native-preset-custom",
    "native-test-again",
    "native-fetch-models",
  ] {
    assert!(!stops.iter().any(|stop| stop == never), "{never} is no stop");
  }
}

#[test]
fn native_connection_saves_a_new_key_secret_and_models_added_by_hand() {
  let mut desktop = Desktop::with_project_named("Kontur");
  add_native(&mut desktop);
  desktop.click("native-key-new");
  desktop.wait_for("native-key-value");
  assert!(
    desktop
      .tree
      .get_element_by_id_mut("native-key-value")
      .and_then(|element| element.as_text_input())
      .expect("the value field")
      .is_masked()
  );
  set(&mut desktop, "native-key-name", "ANTHROPIC_API_KEY");
  set(&mut desktop, "native-key-value", KEY_VALUE);
  assert!(shows(&mut desktop, REASON, "Add at least one model."));
  add_model(&mut desktop, "claude-opus-5-5", ("200000", "64000"), &["high", "max"]);
  assert!(shows(
    &mut desktop,
    "model-row-claude-opus-5-5",
    "Added by hand · 200K context · 64K output · high, max"
  ));
  add_model(&mut desktop, "claude-haiku-4-5", ("", ""), &[]);
  assert!(shows(
    &mut desktop,
    "model-row-claude-haiku-4-5",
    "Added by hand · no effort"
  ));
  choose(&mut desktop, "native-default-model-select", "claude-opus-5-5");
  choose(&mut desktop, "native-default-effort-select", "max");
  assert!(shows(&mut desktop, "model-row-claude-opus-5-5", "Default"));
  desktop.click("native-fallback");
  assert!(!has_reason(&mut desktop), "nothing keeps it from saving");
  desktop.press("connection-save");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  let db = ProjectDb::open(desktop.repo()).expect("open the project");
  let secret = db
    .secrets()
    .expect("secrets")
    .into_iter()
    .find(|secret| secret.name().as_str() == "ANTHROPIC_API_KEY")
    .expect("the key secret is added");
  assert_eq!(secret.kind(), SecretKind::ApiKey);
  assert_eq!(secret.environment(), Environment::Development);
  assert_eq!(desktop.vault.values(), [KEY_VALUE.to_owned()]);
  let connection = db
    .connections()
    .expect("connections")
    .into_iter()
    .find_map(|connection| AgentConnection::try_from(connection).ok())
    .expect("the native connection");
  assert_eq!(connection.provider(), AgentProvider::AnthropicApi);
  assert_eq!(connection.name().as_str(), "Anthropic API · Kontur");
  let api = connection.api().expect("an API endpoint");
  assert_eq!(api.preset(), EndpointPreset::Official);
  assert_eq!(api.base_url().as_str(), "https://api.anthropic.com");
  assert_eq!(api.key(), secret.id());
  assert!(api.provider_fallback());
  let models = api.models();
  assert_eq!(models.models().len(), 2);
  assert_eq!(models.default_model().map(ModelId::as_str), Some("claude-opus-5-5"));
  assert_eq!(models.default_effort(), Some(Effort::Max));
  assert_eq!(models.models()[0].source(), ModelSource::Manual);
  assert_eq!(connection.status(), ConnectionStatus::NotChecked);
  // The value went to the keychain only: nothing on screen holds it.
  assert!(!whole_screen_shows(&mut desktop, KEY_VALUE));
  desktop.wait_for("connection-inspector");
  assert!(shows(
    &mut desktop,
    "connection-inspector",
    "API key · ANTHROPIC_API_KEY (Development)"
  ));
}

/// 04.H3: values the form cannot save say why, under their field and at the left of the actions.
#[test]
fn native_form_says_why_it_cannot_save() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open the project");
    key_secret(&mut db, "ANTHROPIC_API_KEY", Environment::Development, false);
  });
  add_native(&mut desktop);
  desktop.press("native-url-advanced");
  assert!(shows(&mut desktop, DIALOG, "Wire format · Anthropic Messages"));
  assert!(shows(
    &mut desktop,
    DIALOG,
    "https://, or http:// for localhost, 127.0.0.1 or [::1]."
  ));
  for (url, error) in [
    (
      "https://user:key@gateway.example.dev",
      "Keep user names and passwords out of the URL.",
    ),
    ("https://gateway.example.dev/v1?key=1", "Remove the query or fragment."),
    (
      "http://gateway.example.dev",
      "Use https://, or http:// for localhost, 127.0.0.1 or [::1].",
    ),
  ] {
    set(&mut desktop, "native-url", url);
    assert!(shows(&mut desktop, DIALOG, error), "{url}");
    assert!(shows(&mut desktop, REASON, error), "{url}");
  }
  set(&mut desktop, "native-url", "https://gateway.example.dev/v1");
  assert!(shows(&mut desktop, REASON, "Choose or create the API key’s secret."));
  desktop.click("native-key-new");
  set(&mut desktop, "native-key-name", "ANTHROPIC_API_KEY");
  set(&mut desktop, "native-key-value", KEY_VALUE);
  assert!(shows(&mut desktop, REASON, "already has a secret ANTHROPIC_API_KEY."));
  // Back to the project's secrets: the typed value is wiped.
  desktop.click("native-key-existing");
  assert!(!whole_screen_shows(&mut desktop, KEY_VALUE));
  choose(&mut desktop, "native-key-select", "ANTHROPIC_API_KEY (Development)");
  assert!(shows(&mut desktop, REASON, "Add at least one model."));
  // A model id is required, a token count is a whole number, and an id is listed once.
  desktop.click("model-add");
  desktop.wait_for("model-editor");
  set(&mut desktop, "model-id", "lab mini");
  assert!(shows(
    &mut desktop,
    "model-editor",
    "Enter the model id as the API names it"
  ));
  set(&mut desktop, "model-id", "lab-mini-8b");
  set(&mut desktop, "model-context", "lots");
  assert!(shows(&mut desktop, "model-editor", "Enter a whole number of tokens"));
  set(&mut desktop, "model-context", "32000");
  desktop.click("model-save");
  desktop.wait_for("model-row-lab-mini-8b");
  desktop.click("model-add");
  set(&mut desktop, "model-id", "lab-mini-8b");
  assert!(shows(&mut desktop, "model-editor", "lab-mini-8b is listed already."));
  desktop.click("model-cancel");
  assert!(!has_reason(&mut desktop));
  // Testing and fetching stay unavailable whatever is entered.
  desktop.press("native-test-again");
  desktop.click("native-fetch-models");
  assert!(shows(&mut desktop, "native-test", "Not checked"));
}

/// A custom gateway with two models added by hand, the default one used by a Planner profile.
pub(crate) fn seed_lab_gateway(repo: &Path) -> ConnectionId {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let key = key_secret(&mut db, "LAB_GATEWAY_KEY", Environment::Development, false);
  let models = NativeModels::new(
    vec![
      model(
        "lab-mini-8b",
        Some((32_000, 4_000)),
        &[Effort::Low],
        ModelSource::Manual,
      ),
      model("lab-large-70b", None, &[], ModelSource::Manual),
    ],
    Some(ModelId::new("lab-mini-8b").expect("model")),
    Some(Effort::Low),
  )
  .expect("models");
  let id = native_connection(
    &mut db,
    NativeSeed {
      name: "Lab gateway · Kontur",
      provider: AgentProvider::AnthropicApi,
      preset: EndpointPreset::Custom,
      url: "https://lab.example.dev/v1",
      key,
      models,
      check: Some(CheckResult::EndpointUnreachable),
    },
  );
  let permissions = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  );
  profile(&mut db, "Planner", id, "lab-mini-8b", permissions);
  id
}

/// 04.H6: the models dialog edits a model in place, adds one, and keeps a model a profile uses.
#[test]
fn models_dialog_edits_in_place_and_keeps_a_model_a_profile_uses() {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| seeded = Some(seed_lab_gateway(repo)));
  let id: ConnectionId = seeded.expect("seeded");
  open_connections(&mut desktop);
  desktop.click("edit-models");
  desktop.wait_for("models-dialog");
  assert!(shows(&mut desktop, "models-dialog", "Models · Lab gateway · Kontur"));
  assert!(shows(&mut desktop, "model-row-lab-mini-8b", "Default"));
  assert!(shows(
    &mut desktop,
    "model-row-lab-mini-8b",
    "Planner uses lab-mini-8b. Switch to another model before removing it."
  ));
  assert!(!has(&mut desktop, "model-remove-lab-mini-8b"));
  assert!(has(&mut desktop, "model-remove-lab-large-70b"));
  desktop.press("model-edit-lab-large-70b");
  desktop.wait_for("model-editor");
  assert_eq!(desktop.value("model-id"), "lab-large-70b");
  set(&mut desktop, "model-context", "128000");
  desktop.press("model-effort-high");
  desktop.click("model-save");
  desktop.wait_for("model-row-lab-large-70b");
  assert!(shows(&mut desktop, "model-row-lab-large-70b", "128K context · high"));
  desktop.click("model-add");
  set(&mut desktop, "model-id", "lab-tiny-1b");
  desktop.click("model-save");
  desktop.wait_for("model-row-lab-tiny-1b");
  desktop.press("model-remove-lab-tiny-1b");
  assert!(!has(&mut desktop, "model-row-lab-tiny-1b"));
  desktop.press("models-save");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, "models-dialog"));
  let stored = AgentConnection::try_from(
    ProjectDb::open(desktop.repo())
      .expect("open the project")
      .connection(id)
      .expect("read")
      .expect("stored"),
  )
  .expect("an agent connection");
  let models = stored.api().expect("an API endpoint").models();
  let ids: Vec<&str> = models.models().iter().map(|model| model.id().as_str()).collect();
  assert_eq!(ids, ["lab-mini-8b", "lab-large-70b"]);
  assert_eq!(models.default_model().map(ModelId::as_str), Some("lab-mini-8b"));
  assert_eq!(models.default_effort(), Some(Effort::Low));
  assert_eq!(models.models()[1].efforts(), [Effort::High]);
  // Its key, endpoint and last test stay.
  assert_eq!(stored.status(), ConnectionStatus::EndpointUnreachable);
}

#[test]
fn editing_a_native_connection_keeps_its_key_models_and_last_test() {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open the project");
    let key = key_secret(&mut db, "ANTHROPIC_API_KEY", Environment::Staging, false);
    let models = NativeModels::new(
      vec![model("claude-opus-5-5", None, &[Effort::Medium], ModelSource::Manual)],
      None,
      None,
    )
    .expect("models");
    seeded = Some(native_connection(
      &mut db,
      NativeSeed {
        name: "Claude API · Kontur",
        provider: AgentProvider::AnthropicApi,
        preset: EndpointPreset::Official,
        url: "https://api.anthropic.com",
        key,
        models,
        check: Some(CheckResult::KeyRejected),
      },
    ));
  });
  let id = seeded.expect("seeded");
  open_connections(&mut desktop);
  desktop.click("edit-connection");
  desktop.wait_for("native-preset");
  assert!(
    !has(&mut desktop, "connection-kind-anthropic"),
    "a connection keeps its kind"
  );
  assert_eq!(desktop.value("connection-name"), "Claude API · Kontur");
  assert!(shows(&mut desktop, "native-key", "ANTHROPIC_API_KEY (Staging)"));
  assert!(shows(&mut desktop, "native-test", "Key rejected"));
  assert!(has(&mut desktop, "model-row-claude-opus-5-5"));
  set(&mut desktop, "connection-name", "Claude API · Main");
  desktop.press("connection-save");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  let stored = ProjectDb::open(desktop.repo())
    .expect("open the project")
    .connection(id)
    .expect("read")
    .expect("stored");
  assert_eq!(stored.name().as_str(), "Claude API · Main");
  assert_eq!(stored.status(), ConnectionStatus::KeyRejected);
}
