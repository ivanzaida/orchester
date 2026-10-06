//! The OpenAI API kind of a native connection (04.H1 `Byo8K`): added as the Anthropic one is, and its
//! test and model fetch unavailable while Orchester runs no OpenAI API agents. Key values here are fake,
//! and the keychain is the harness's one in memory.

use orchester_core::{
  connection::{AgentConnection, AgentProvider, Effort, EndpointPreset},
  store::ConnectionStore,
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  native_connections::{open_connections, seeded},
  native_dialog::{DIALOG, KEY_VALUE, add_model, choose, has_reason, set, whole_screen_shows},
  task_run::has,
};

/// The OpenAI kind is added as the Anthropic one is: the Official preset at OpenAI's base URL, the
/// Responses wire format, a key secret and models by hand; testing and fetching models stay
/// unavailable until OpenAI API agents run, and provider fallback is Anthropic's only.
#[test]
fn native_openai_connection_is_added_like_the_anthropic_one() {
  let mut desktop = Desktop::with_project_named("Kontur");
  desktop.wait_for("nav-connections");
  desktop.click("nav-connections");
  desktop.wait_for("add-connection");
  desktop.click("add-connection");
  desktop.wait_for(DIALOG);
  desktop.press("connection-kind-openai");
  desktop.wait_for("native-preset");
  assert_eq!(desktop.value("connection-name"), "OpenAI API · Kontur");
  assert!(shows(&mut desktop, "native-url", "https://api.openai.com/v1"));
  assert!(shows(
    &mut desktop,
    "native-preset",
    "Compatible endpoints come in a later version of Orchester."
  ));
  for text in [
    "Set by the preset. Prompts, repository content and command output are sent to api.openai.com.",
    "Testing an OpenAI API connection comes with its agent runs in a later version.",
    "Fetching models from an OpenAI API endpoint comes with its agent runs in a later version.",
    "Not offered on this endpoint: only the official Anthropic API retries on another model.",
  ] {
    assert!(shows(&mut desktop, DIALOG, text), "{text}");
  }
  assert!(!whole_screen_shows(&mut desktop, "Anthropic may answer"));
  desktop.press("native-url-advanced");
  assert!(shows(
    &mut desktop,
    DIALOG,
    "Wire format · OpenAI Responses · stateless, store off"
  ));
  desktop.click("native-key-new");
  desktop.wait_for("native-key-name");
  let placeholder = desktop
    .tree
    .get_element_by_id_mut("native-key-name")
    .and_then(|element| element.as_text_input())
    .and_then(|input| input.placeholder());
  assert_eq!(placeholder.as_deref(), Some("OPENAI_API_KEY"));
  set(&mut desktop, "native-key-name", "OPENAI_API_KEY");
  set(&mut desktop, "native-key-value", KEY_VALUE);
  add_model(
    &mut desktop,
    "gpt-6.1-sol",
    ("1050000", "128000"),
    &["low", "medium", "high"],
  );
  choose(&mut desktop, "native-default-model-select", "gpt-6.1-sol");
  choose(&mut desktop, "native-default-effort-select", "medium");
  assert!(!has_reason(&mut desktop), "nothing keeps it from saving");
  desktop.press("connection-save");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  let db = ProjectDb::open(desktop.repo()).expect("open the project");
  let connection = db
    .connections()
    .expect("connections")
    .into_iter()
    .find_map(|connection| AgentConnection::try_from(connection).ok())
    .expect("the native connection");
  assert_eq!(connection.provider(), AgentProvider::OpenAiApi);
  let api = connection.api().expect("an API endpoint");
  assert_eq!(api.preset(), EndpointPreset::Official);
  assert_eq!(api.base_url().as_str(), "https://api.openai.com/v1");
  assert!(!api.provider_fallback());
  assert_eq!(api.models().default_effort(), Some(Effort::Medium));
  assert_eq!(desktop.vault.values(), [KEY_VALUE.to_owned()]);
  assert!(!whole_screen_shows(&mut desktop, KEY_VALUE));
}

/// Orchester does not run OpenAI API agents yet: the dialog's Test again and Fetch models stay
/// unavailable and say why, and pressing them reaches no runner.
#[test]
fn native_dialog_keeps_testing_an_openai_connection_unavailable() {
  let (mut desktop, ..) = seeded(AgentProvider::OpenAiApi);
  open_connections(&mut desktop);
  desktop.click("edit-connection");
  desktop.wait_for("native-preset");
  let unavailable = "Testing an OpenAI API connection comes with its agent runs in a later version.";
  assert!(shows(&mut desktop, "native-test", unavailable));
  assert!(shows(
    &mut desktop,
    DIALOG,
    "Fetching models from an OpenAI API endpoint comes with its agent runs in a later version."
  ));
  assert!(!whole_screen_shows(&mut desktop, "Fetching tests the connection"));
  desktop.press("native-test-again");
  desktop.press("native-fetch-models");
  assert!(shows(&mut desktop, "native-test", unavailable));
  assert!(!shows(
    &mut desktop,
    "native-test",
    "The project’s runner is not connected"
  ));
}
