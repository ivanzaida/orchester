//! A team start whose bundle this computer cannot read is refused with the reason and the side to update
//! (`appMcp.errors.teamBundleSchema*`, `teamBundleUnreadable`): nothing waits for a bundle that reads the same every
//! time.

use orchester_api::v2::EXECUTION_BUNDLE_SCHEMA;
use orchester_core::store::McpServerStore;
use serde_json::{Value, json};

use crate::{
  Checkout,
  pipelines::team_pipeline,
  team_server::TeamServer,
  tools::{local_mcp_server, server},
};

/// A checkout whose task can start: its pipeline and the Builder's MCP server are here. `prepare` runs on the fake
/// server's state before `run_task` is called.
pub(crate) fn started_with(prepare: impl FnOnce(&mut crate::team_server::State)) -> (TeamServer, Checkout) {
  let (server, seen) = server();
  let pipeline = server.state().config.pipeline;
  let mcp_server = seen.lock().publisher_mcp_server.expect("the Builder's MCP server");
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    team_pipeline(db, pipeline, "delivery");
    db.create_mcp_server(&local_mcp_server(mcp_server))
      .expect("a local MCP server");
  });
  prepare(&mut server.state());
  (server, checkout)
}

/// The refusal of `run_task` on KON-7: its code and the error, each message rendered as `key read=… built=…`.
pub(crate) fn refusal(checkout: &Checkout) -> (String, Value) {
  let error = checkout
    .call_rendering("run_task", &json!({"project": "Kontur", "key": "KON-7"}), |message| {
      let values: Vec<String> = message
        .args
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
      format!("{} {}", message.key, values.join(" "))
    })
    .expect_err("refused");
  let code = error["error"]["code"].as_str().expect("a code").to_owned();
  (code, error)
}

fn assert_schema_refusal(error: &Value, key: &str, (read, built): (u32, u32)) {
  let rendered = error.to_string();
  assert!(rendered.contains(key), "{rendered}");
  assert!(
    rendered.contains(&read.to_string()) && rendered.contains(&built.to_string()),
    "{rendered}"
  );
}

#[test]
fn team_mcp_bundle_schema_older_than_this_app_names_the_server_as_the_one_to_update() {
  let older = EXECUTION_BUNDLE_SCHEMA - 1;
  let (server, checkout) = started_with(|state| state.receipt_schema = Some(older));
  let (code, error) = refusal(&checkout);
  assert_eq!(code, "team_bundle_schema_mismatch", "{error}");
  assert_schema_refusal(
    &error,
    "appMcp.errors.teamBundleSchemaOlder",
    (EXECUTION_BUNDLE_SCHEMA, older),
  );
  assert_eq!(server.state().works_made, 1);
}

#[test]
fn team_mcp_bundle_schema_newer_than_this_app_names_this_app_as_the_one_to_update() {
  let newer = EXECUTION_BUNDLE_SCHEMA + 1;
  let (_server, checkout) = started_with(|state| state.receipt_schema = Some(newer));
  let (code, error) = refusal(&checkout);
  assert_eq!(code, "team_bundle_schema_mismatch", "{error}");
  assert_schema_refusal(
    &error,
    "appMcp.errors.teamBundleSchemaNewer",
    (EXECUTION_BUNDLE_SCHEMA, newer),
  );
}

#[test]
fn team_mcp_bundle_that_is_json_but_no_bundle_is_refused_unreadable() {
  let (_server, checkout) = started_with(|state| state.bundle_body = Some(json!({"error": "gateway"})));
  let (code, error) = refusal(&checkout);
  assert_eq!(code, "team_bundle_unreadable", "{error}");
  assert!(
    error.to_string().contains("appMcp.errors.teamBundleUnreadable"),
    "{error}"
  );
}
