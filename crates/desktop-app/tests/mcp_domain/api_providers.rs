//! The person's API providers through the domain tools (`docs/design/agent-accounts.md`, "Agents run on accounts"):
//! a provider is added by a person in Settings › Accounts and listed without its key; running a profile on one
//! sends the project's work to its host, a person's decision unless they let automation widen; and no result holds
//! the key.

use std::sync::Arc;

use orchester_core::{
  account::{AccountCli, ApiAccount, ApiSetup},
  connection::BaseUrl,
  name::Name,
  resource::ModelId,
  secret::SecretValue,
};
use serde_json::{Value, json};

use super::{Served, vault::MemoryVault, write_program};

/// The key the vault holds; no result may hold it.
const KEY: &str = "sk-fake-provider-domain-2b9c47e1";

/// The connections scenario served on a vault, with the API provider *DeepSeek* of Claude Code added as a person
/// adds it in Settings › Accounts, and a file chosen as Claude Code's program on this computer.
fn served() -> (Served, ApiAccount) {
  let vault = Arc::new(MemoryVault::default());
  let (served, _) = Served::seeded_on_vault("connections", vault.clone());
  let models = ["deepseek-flash", "deepseek-v4-pro"]
    .into_iter()
    .map(|model| ModelId::new(model).expect("a model id"))
    .collect();
  let setup = ApiSetup::cli(
    AccountCli::ClaudeCode,
    BaseUrl::new("https://api.deepseek.com/anthropic").expect("a base URL"),
    models,
  )
  .expect("a setup");
  let workspace = &served.services().workspace;
  let account = workspace
    .add_api_account(
      vault.as_ref(),
      Name::new("DeepSeek").expect("a name"),
      setup,
      &SecretValue::new(KEY.to_owned()).expect("a value"),
    )
    .expect("the provider is added");
  let program = served.folder.path().join("my-claude.exe");
  write_program(&program, 0o755);
  workspace
    .set_agent_program(AccountCli::ClaudeCode, &program)
    .expect("the program is kept");
  (served, account)
}

fn assert_no_key(text: &str) {
  assert!(!text.contains(KEY), "a result holds the key: {text}");
}

#[test]
fn domain_api_providers_are_listed_without_their_key_and_serve_the_models_typed_for_them() {
  let (mut served, account) = served();
  let listed = served.ok("list_accounts", &json!({}));
  assert_no_key(&listed.to_string());
  let provider = listed["api_providers"]
    .as_array()
    .and_then(|providers| providers.iter().find(|provider| provider["name"] == "DeepSeek"))
    .unwrap_or_else(|| panic!("DeepSeek is listed: {listed}"));
  assert_eq!(provider["id"], account.id().to_string());
  assert_eq!(provider["used_with"], "claude_code");
  assert_eq!(provider["host"], "api.deepseek.com");
  assert_eq!(provider["key_here"], true, "{provider}");
  assert_eq!(provider["status"], Value::Null, "an agent CLI's provider is not tested");
  let programs = listed["programs"].as_array().expect("each CLI's program");
  let claude = programs
    .iter()
    .find(|program| program["cli"] == "claude_code")
    .expect("Claude Code's");
  assert!(
    claude["set"].as_str().is_some_and(|set| set.ends_with("my-claude.exe")),
    "{claude}"
  );

  // The project has no connection on it yet: its models are the ones typed for it.
  let models = served.ok("list_models", &json!({ "project": "Kontur", "account": "deepseek" }));
  assert_eq!(models["source"], "api_provider", "{models}");
  assert_eq!(models["models"][1]["id"], "deepseek-v4-pro", "{models}");
  let refused = served.refused("check_account", &json!({ "account": "DeepSeek" }));
  assert_eq!(refused["code"], "not_tested", "{refused}");
}

#[test]
fn domain_api_providers_take_a_profile_only_while_a_person_lets_automation_widen() {
  let (mut served, account) = served();
  // A profile on Claude Code's own sign-in, with a model the provider serves too.
  served.let_automation_widen(true);
  served.ok(
    "save_profile_draft",
    &json!({
      "project": "Kontur", "role": "Runner", "account": "default", "cli": "claude_code", "model": "deepseek-flash",
    }),
  );
  served.let_automation_widen(false);
  let on = json!({ "project": "Kontur", "profile": "Runner", "account": "DeepSeek" });
  let refused = served.refused("set_profile_account", &on);
  assert_eq!(refused["code"], "person_only", "{refused}");
  assert!(refused["reasons"].to_string().contains("api.deepseek.com"), "{refused}");

  served.let_automation_widen(true);
  let set = served.ok("set_profile_account", &on);
  assert_no_key(&set.to_string());
  assert_eq!(set["api_provider"]["id"], account.id().to_string(), "{set}");
  let connections = served.ok("list_connections", &json!({ "project": "Kontur" }));
  let row = connections["runs_on"]
    .as_array()
    .and_then(|rows| rows.iter().find(|row| row["profile"]["role"] == "Runner"))
    .unwrap_or_else(|| panic!("Runner is listed: {connections}"));
  assert_eq!(row["runs_on"]["api_provider"]["name"], "DeepSeek", "{row}");
  assert_eq!(row["runs_on"]["account"], Value::Null, "{row}");

  // The provider it runs on already is no widening.
  served.let_automation_widen(false);
  served.ok("set_profile_account", &on);
  for tool in ["list_connections", "list_profiles", "list_secrets", "get_project"] {
    let text = match served.call(tool, &json!({ "project": "Kontur" })) {
      Ok(value) | Err(value) => value.to_string(),
    };
    assert_no_key(&text);
  }
  // Back on the CLI's own sign-in: its Default is never a widening.
  let back = served.ok(
    "set_profile_account",
    &json!({ "project": "Kontur", "profile": "Runner", "account": "default" }),
  );
  assert_eq!(back["account"]["default"], true, "{back}");
}
