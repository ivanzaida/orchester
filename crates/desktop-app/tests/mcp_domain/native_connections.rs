//! Orchester's own agent through the domain tools holds the window's rules (`docs/architecture/native-agent.md` §7):
//! its model API is one of the person's API providers, whose models are the ones it stores, with each model's
//! efforts; a profile on one takes only an effort its model lists; and `check_account` tests it with its key,
//! refused where the keychain does not have it.

use std::sync::Arc;

use orchester_core::{
  id::ApiAccountId,
  secret::{SecretValue, SecretVault, VaultKey},
  store::SecretStore,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{Served, vault::MemoryVault};

/// The native scenario with its connection's key on this computer, taken over as the person's API provider as
/// opening the project does; the vault and the provider's id.
fn served() -> (Served, Arc<MemoryVault>, String) {
  let vault = Arc::new(MemoryVault::default());
  let (mut served, seeded) = Served::seeded_on_vault("native", vault.clone());
  let db = ProjectDb::open(&seeded.repo).expect("the project database");
  let project = db.project().id();
  let value = SecretValue::new("sk-ant-fake-native-domain".to_owned()).expect("a value");
  for secret in db.secrets().expect("the project's secrets") {
    if !secret.is_archived() {
      vault
        .store(&secret.vault_key(project), &value)
        .expect("the key is on this computer");
    }
  }
  drop(db);
  served
    .services()
    .workspace
    .adopt_project_keys(&seeded.repo, vault.as_ref())
    .expect("the project's keys become the app's");
  let listed = served.ok("list_accounts", &json!({}));
  let provider = listed["api_providers"]
    .as_array()
    .and_then(|providers| {
      providers
        .iter()
        .find(|provider| provider["used_with"] == "orchester_agent")
    })
    .unwrap_or_else(|| panic!("the model API of Orchester's own agent is listed: {listed}"));
  assert_eq!(provider["models"].as_array().map(Vec::len), Some(3), "{provider}");
  let id = provider["id"].as_str().expect("an id").to_owned();
  (served, vault, id)
}

fn on_native(id: &str) -> Value {
  json!({ "project": "Kontur", "account": id })
}

#[test]
fn domain_native_models_are_the_stored_ones() {
  let (mut served, _vault, id) = served();
  let listed = served.ok("list_models", &on_native(&id));
  assert_eq!(listed["source"], "native", "{listed}");
  assert_eq!(
    listed["models"],
    json!([
      { "id": "claude-opus-5-5", "efforts": ["low", "medium", "high", "max"] },
      { "id": "claude-sonnet-5-5", "efforts": ["low", "medium", "high"] },
      { "id": "claude-review-preview", "efforts": [] },
    ]),
    "{listed}"
  );
}

#[test]
fn domain_native_profile_takes_only_an_effort_its_model_lists() {
  let (mut served, _vault, id) = served();
  let builder = |effort: &str| {
    json!({
      "project": "Kontur",
      "profile": "Builder",
      "role": "Builder",
      "account": id,
      "model": "claude-opus-5-5",
      "effort": effort,
      "permissions": { "repository": "write", "commands": "ask_before", "network": "off" },
    })
  };
  let refused = served.refused("save_profile_draft", &builder("bogus"));
  assert_eq!(refused["code"], "effort_not_offered", "{refused}");
  let saved = served.ok("save_profile_draft", &builder("max"));
  assert_eq!(saved["profile"]["role"], "Builder", "{saved}");
}

#[test]
fn domain_native_check_needs_the_key_on_this_computer() {
  let (mut served, vault, id) = served();
  let account: ApiAccountId = id.parse().expect("an account id");
  vault
    .delete(&VaultKey::api_account(account))
    .expect("the key leaves the keychain");
  let refused = served.refused("check_account", &json!({ "account": id }));
  assert_eq!(refused["code"], "key_missing", "{refused}");
  assert!(
    !refused.to_string().contains("sk-ant-fake"),
    "no key in a refusal: {refused}"
  );
}
