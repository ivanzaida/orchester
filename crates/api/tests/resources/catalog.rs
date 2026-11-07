//! The CLI kinds and models per role catalog (row 75): the report and the two reads round-trip, none takes a field
//! beyond CLI kinds and model ids, and the report's checks hold.

use orchester_api::{CATALOG_CLI_KINDS, CatalogCliKinds, CatalogModels, CatalogRefused, CatalogReport};
use serde_json::{Value, json};

use crate::fixtures::round_trip;

fn models() -> Value {
  json!({"claude_code_cli":["claude-opus-5-5","claude-sonnet-5-5"],"openai_api":["gpt-5.5"]})
}

fn report(cli_kinds: &[&str], models: &Value) -> CatalogReport {
  serde_json::from_value(json!({"cli_kinds":cli_kinds,"models":models})).expect("a report")
}

#[test]
fn catalog_a_report_round_trips_with_cli_kinds_and_models_per_role() {
  let report: CatalogReport = round_trip(&json!({"cli_kinds":["claude_code_cli","openai_api"],"models":models()}));
  assert_eq!(report.cli_kinds, ["claude_code_cli", "openai_api"]);
  let claude: Vec<_> = report.models["claude_code_cli"]
    .iter()
    .map(|model| model.as_str())
    .collect();
  assert_eq!(claude, ["claude-opus-5-5", "claude-sonnet-5-5"]);
  round_trip::<CatalogReport>(&json!({"cli_kinds":[],"models":{}}));
}

#[test]
fn catalog_the_reads_round_trip() {
  let listed: CatalogCliKinds = round_trip(&json!({"cli_kinds":["claude_code_cli","codex_cli"]}));
  assert_eq!(listed.cli_kinds.len(), 2);
  let models: CatalogModels = round_trip(&json!({"models":models()}));
  assert_eq!(models.models["openai_api"].len(), 1);
}

#[test]
fn catalog_the_old_references_body_is_not_a_report() {
  let old = json!({"references":[{"kind":"mcp_server","name":"GitHub","role":null}],"models":{}});
  assert!(serde_json::from_value::<CatalogReport>(old).is_err());
  let old = json!({"cli_kinds":[],"references":[],"models":{}});
  assert!(serde_json::from_value::<CatalogReport>(old).is_err());
  assert!(serde_json::from_value::<CatalogCliKinds>(json!({"references":[]})).is_err());
}

#[test]
fn catalog_a_model_id_is_a_token() {
  for id in ["", "two words"] {
    let report = json!({"cli_kinds":[],"models":{"codex_cli":[id]}});
    assert!(serde_json::from_value::<CatalogReport>(report).is_err(), "{id:?}");
  }
}

#[test]
fn catalog_a_report_of_the_four_agent_roles_holds() {
  let all = ["codex_cli", "claude_code_cli", "anthropic_api", "openai_api"];
  assert_eq!(all.len(), CATALOG_CLI_KINDS);
  let models = json!({"codex_cli":["gpt-5.5"],"anthropic_api":["claude-opus-5-5"]});
  assert_eq!(report(&all, &models).check(), Ok(()));
  assert_eq!(report(&[], &json!({})).check(), Ok(()));
}

#[test]
fn catalog_github_or_an_unknown_role_is_refused_as_a_cli_kind_and_as_a_model_role() {
  for role in ["github", "gemini_cli", "", "Codex_cli"] {
    let kind = report(&["codex_cli", role], &json!({}));
    assert_eq!(
      kind.check(),
      Err(CatalogRefused::NotAnAgentRole(role.to_owned())),
      "{role:?}"
    );
    let models = report(&[], &json!({role:["gpt-5.5"]}));
    assert_eq!(
      models.check(),
      Err(CatalogRefused::NotAnAgentRole(role.to_owned())),
      "{role:?}"
    );
  }
}

#[test]
fn catalog_a_report_over_the_limits_is_refused() {
  let five = [
    "codex_cli",
    "claude_code_cli",
    "anthropic_api",
    "openai_api",
    "codex_cli",
  ];
  assert_eq!(report(&five, &json!({})).check(), Err(CatalogRefused::TooManyCliKinds));

  let ids = |count: usize| (0..count).map(|number| format!("model-{number}")).collect::<Vec<_>>();
  assert_eq!(report(&[], &json!({"codex_cli":ids(512)})).check(), Ok(()));
  assert_eq!(
    report(&[], &json!({"codex_cli":ids(513)})).check(),
    Err(CatalogRefused::TooManyModels)
  );
}

#[test]
fn catalog_no_shape_takes_a_name_account_path_setting_or_key() {
  let extras = [
    ("references", json!([])),
    ("name", json!("Work")),
    ("account", json!("alice@example.com")),
    ("path", json!("C:/Users/alice/.claude")),
    ("settings", json!({"model":"claude-opus-5-5"})),
    ("key", json!("sk-secret")),
    ("id", json!("0192f2c4-0000-7000-8000-000000000000")),
    ("device", json!("laptop")),
  ];
  for (field, value) in extras {
    let mut report = json!({"cli_kinds":[],"models":{}});
    report[field] = value.clone();
    assert!(serde_json::from_value::<CatalogReport>(report).is_err(), "{field}");
    let mut listed = json!({"cli_kinds":[]});
    listed[field] = value.clone();
    assert!(serde_json::from_value::<CatalogCliKinds>(listed).is_err(), "{field}");
    let mut models = json!({"models":{}});
    models[field] = value;
    assert!(serde_json::from_value::<CatalogModels>(models).is_err(), "{field}");
  }
}
