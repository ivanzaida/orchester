//! The references catalog and models per role (row 75): the report and the two reads round-trip, and none of them
//! takes a field beyond kinds, names, roles and model ids.

use orchester_api::{CatalogKind, CatalogModels, CatalogReference, CatalogReferences, CatalogReport};
use serde_json::{Value, json};

use crate::fixtures::round_trip;

fn references() -> Value {
  json!([
    {"kind":"connection","name":"Work","role":"claude_code_cli"},
    {"kind":"connection","name":"Team key","role":"openai_api"},
    {"kind":"mcp_server","name":"GitHub","role":null},
  ])
}

fn models() -> Value {
  json!({"claude_code_cli":["claude-opus-5-5","claude-sonnet-5-5"],"openai_api":["gpt-5.5"]})
}

#[test]
fn catalog_a_report_round_trips_with_references_and_models_per_role() {
  let report: CatalogReport = round_trip(&json!({"references":references(),"models":models()}));
  let kinds: Vec<_> = report.references.iter().map(|reference| reference.kind).collect();
  assert_eq!(
    kinds,
    [CatalogKind::Connection, CatalogKind::Connection, CatalogKind::McpServer]
  );
  assert_eq!(report.references[2].role, None);
  let claude: Vec<_> = report.models["claude_code_cli"]
    .iter()
    .map(|model| model.as_str())
    .collect();
  assert_eq!(claude, ["claude-opus-5-5", "claude-sonnet-5-5"]);
  round_trip::<CatalogReport>(&json!({"references":[],"models":{}}));
}

#[test]
fn catalog_the_reads_round_trip() {
  let listed: CatalogReferences = round_trip(&json!({"references":references()}));
  assert_eq!(listed.references.len(), 3);
  let models: CatalogModels = round_trip(&json!({"models":models()}));
  assert_eq!(models.models["openai_api"].len(), 1);
}

#[test]
fn catalog_an_mcp_server_may_leave_its_role_out() {
  let reference: CatalogReference = serde_json::from_value(json!({"kind":"mcp_server","name":"GitHub"})).expect("read");
  assert_eq!(reference.role, None);
}

#[test]
fn catalog_a_reference_needs_a_known_kind_and_a_name() {
  for value in [
    json!({"kind":"secret","name":"API token","role":"token"}),
    json!({"kind":"tool","name":"Deploy","role":null}),
    json!({"kind":"connection","role":"codex_cli"}),
    json!({"name":"Work","role":"codex_cli"}),
  ] {
    assert!(serde_json::from_value::<CatalogReference>(value).is_err());
  }
}

#[test]
fn catalog_a_model_id_is_a_token() {
  for id in ["", "two words"] {
    let report = json!({"references":[],"models":{"codex_cli":[id]}});
    assert!(serde_json::from_value::<CatalogReport>(report).is_err(), "{id:?}");
  }
}

#[test]
fn catalog_no_shape_takes_an_account_path_setting_or_key() {
  let extras = [
    ("account", json!("alice@example.com")),
    ("path", json!("C:/Users/alice/.claude")),
    ("settings", json!({"model":"claude-opus-5-5"})),
    ("key", json!("sk-secret")),
    ("id", json!("0192f2c4-0000-7000-8000-000000000000")),
    ("device", json!("laptop")),
  ];
  for (field, value) in extras {
    let mut reference = references()[0].clone();
    reference[field] = value.clone();
    assert!(
      serde_json::from_value::<CatalogReference>(reference).is_err(),
      "{field}"
    );
    let mut report = json!({"references":[],"models":{}});
    report[field] = value.clone();
    assert!(serde_json::from_value::<CatalogReport>(report).is_err(), "{field}");
    let mut listed = json!({"references":[]});
    listed[field] = value.clone();
    assert!(serde_json::from_value::<CatalogReferences>(listed).is_err(), "{field}");
    let mut models = json!({"models":{}});
    models[field] = value;
    assert!(serde_json::from_value::<CatalogModels>(models).is_err(), "{field}");
  }
}
