//! Agent CLIs on API providers through the domain tools (`docs/design/api-providers.md`): running a connection on a
//! provider's host or key is a person's decision unless they let automation widen, going back to the sign-in or
//! changing only the models is not; the provider names its key secret, and no result holds the key.

use std::sync::Arc;

use orchester_core::secret::{Environment, NewSecret, SecretKind, SecretName, SecretValue};
use orchester_workspace::secrets::data::add_in;
use serde_json::{Value, json};

use super::{Served, vault::MemoryVault};

/// The key the vault holds; no result may hold it.
const KEY: &str = "sk-fake-provider-domain-2b9c47e1";

/// The connections scenario served on a vault holding the key in secret DEEPSEEK_KEY.
fn served() -> Served {
  let vault = Arc::new(MemoryVault::default());
  let (served, seeded) = Served::seeded_on_vault("connections", vault.clone());
  let new = NewSecret {
    name: SecretName::new("DEEPSEEK_KEY").expect("a secret name"),
    kind: SecretKind::ApiKey,
    environment: Environment::Development,
  };
  let value = SecretValue::new(KEY.to_owned()).expect("a value");
  add_in(&seeded.repo, vault.as_ref(), new, &value).expect("the key secret");
  served
}

fn on(url: &str, models: &[&str]) -> Value {
  json!({
    "project": "Kontur",
    "connection": "Claude · Kontur",
    "api_provider": { "base_url": url, "key_secret": "DEEPSEEK_KEY", "models": models },
  })
}

fn assert_person_only(error: &Value) {
  assert_eq!(error["code"], "person_only", "{error}");
  let reasons = error["reasons"].to_string();
  assert!(
    reasons.contains("api.deepseek.com") && reasons.contains("DEEPSEEK_KEY"),
    "{error}"
  );
}

fn assert_no_key(text: &str) {
  assert!(!text.contains(KEY), "a result holds the key: {text}");
}

#[test]
fn domain_api_providers_need_a_person_for_a_new_host_or_key_and_never_hold_the_key() {
  let mut served = served();
  let deepseek = on("https://api.deepseek.com/anthropic", &["deepseek-flash"]);
  assert_person_only(&served.refused("set_api_provider", &deepseek));

  served.let_automation_widen(true);
  let set = served.ok("set_api_provider", &deepseek);
  assert_no_key(&set.to_string());
  let connection = &set["connection"];
  assert_eq!(
    connection["status"], "connected",
    "the CLI's own sign-in is not used: {connection}"
  );
  assert_eq!(connection["account"], Value::Null, "{connection}");
  assert_eq!(
    connection["api_provider"],
    json!({
      "base_url": "https://api.deepseek.com/anthropic",
      "host": "api.deepseek.com",
      "key_secret": "DEEPSEEK_KEY",
      "models": ["deepseek-flash"],
    })
  );

  served.let_automation_widen(false);
  let models = served.ok(
    "set_api_provider",
    &on(
      "https://api.deepseek.com/anthropic",
      &["deepseek-flash", "deepseek-v4-pro"],
    ),
  );
  assert_eq!(
    models["connection"]["api_provider"]["models"],
    json!(["deepseek-flash", "deepseek-v4-pro"]),
    "only the models change: no widening"
  );
  let listed = served.ok(
    "list_models",
    &json!({ "project": "Kontur", "connection": "Claude · Kontur" }),
  );
  assert_eq!(listed["source"], "api_provider", "{listed}");
  assert_eq!(listed["models"][1]["id"], "deepseek-v4-pro", "{listed}");
  let elsewhere = on("https://gateway.example.com", &["deepseek-flash"]);
  assert_eq!(served.refused("set_api_provider", &elsewhere)["code"], "person_only");

  for tool in ["list_connections", "list_secrets", "get_project"] {
    let text = match served.call(tool, &json!({ "project": "Kontur" })) {
      Ok(value) | Err(value) => value.to_string(),
    };
    assert_no_key(&text);
  }

  let back = served.ok(
    "set_api_provider",
    &json!({ "project": "Kontur", "connection": "Claude · Kontur", "api_provider": null }),
  );
  assert_eq!(
    back["connection"]["api_provider"],
    Value::Null,
    "going back is a narrowing"
  );
  assert_eq!(back["connection"]["status"], "sign_in_required");
}

#[test]
fn domain_api_providers_are_checked_as_the_form_checks_them() {
  let mut served = served();
  served.let_automation_widen(true);
  let refused = |served: &mut Served, args: &Value| served.refused("set_api_provider", args)["code"].clone();
  assert_eq!(
    refused(&mut served, &on("http://api.deepseek.com", &["deepseek-flash"])),
    "invalid_base_url"
  );
  assert_eq!(
    refused(&mut served, &on("https://api.deepseek.com", &[])),
    "invalid_api_provider"
  );
  assert_eq!(
    refused(&mut served, &on("https://api.deepseek.com", &["deep seek"])),
    "invalid_model"
  );
  let mut unknown = on("https://api.deepseek.com", &["deepseek-flash"]);
  unknown["api_provider"]["key_secret"] = json!("NO_SUCH_SECRET");
  assert_eq!(refused(&mut served, &unknown), "secret_not_found");
  let mut github = on("https://api.deepseek.com", &["deepseek-flash"]);
  github["connection"] = json!("GitHub · github.com");
  assert_eq!(refused(&mut served, &github), "not_an_agent_connection");
}

#[test]
fn domain_api_providers_a_cli_added_on_a_provider_names_both_widenings() {
  let mut served = served();
  let program = served.folder.path().join("my-claude.exe");
  let args = json!({
    "project": "Kontur",
    "kind": "claude",
    "name": "DeepSeek · Claude",
    "program": program.display().to_string(),
    "api_provider": {
      "base_url": "https://api.deepseek.com/anthropic",
      "key_secret": "DEEPSEEK_KEY",
      "models": ["deepseek-flash"],
    },
  });
  let refused = served.refused("add_cli_connection", &args);
  assert_person_only(&refused);
  served.let_automation_widen(true);
  assert_eq!(
    served.refused("add_cli_connection", &args)["code"],
    "not_verified",
    "the program is checked once a person lets it, and saved only when it is the CLI"
  );
}
