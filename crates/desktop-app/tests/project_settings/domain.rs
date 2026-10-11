//! JSON argument parsing and domain dispatch, without creating MCP discovery credentials.
use orchester_workspace::app_mcp::{self, ToolError, Translate};
use serde_json::{Value, json};

use super::harness::Settings;

pub(super) fn call(page: &Settings, name: &str, args: Value) -> Result<Value, ToolError> {
  let s = page.window.launch.services().expect("launched services");
  let services = app_mcp::Services::new(
    s.workspace.clone(),
    s.runners.clone(),
    s.servers.clone(),
    s.programs.clone(),
    (s.changes.clone(), s.activity.clone(), s.jobs.clone()),
  );
  let translate = Translate::new(|message| message.key.to_owned());
  page.window.data.runtime.block_on(async {
    let Value::Object(args) = args else {
      panic!("arguments must be an object")
    };
    if let Some(result) = app_mcp::call(&services, &translate, name, args.clone()).await {
      return result;
    }
    app_mcp::setup::call(&services, &translate, name, args)
      .await
      .expect("domain setup tool")
  })
}

#[test]
fn project_settings_domain_limits_accept_null_daily_budget_and_preserve_it_when_omitted() {
  let page = Settings::new(false);
  let unlimited = call(
    &page,
    "update_project_limits",
    json!({
      "project": "Kontur", "daily_budget": null, "default_run_budget": 900000000,
      "default_time_limit_minutes": null
    }),
  )
  .expect("unlimited");
  assert!(unlimited["limits"]["daily_budget"].is_null());
  assert_eq!(unlimited["limits"]["default_run_budget"], 900000000);
  let changed = call(
    &page,
    "update_project_limits",
    json!({"project":"Kontur", "agent_slots":16}),
  )
  .expect("slots");
  assert!(changed["limits"]["daily_budget"].is_null());
  assert_eq!(changed["limits"]["agent_slots"], 16);
  let read = call(&page, "get_project", json!({"project":"Kontur"})).expect("readback");
  assert_eq!(read["project"]["limits"]["agent_slots"], 16);
  assert_eq!(read["project"]["limits"]["default_run_budget"], 900000000);
  assert!(read["project"]["limits"]["daily_budget"].is_null());
  let restored = call(
    &page,
    "update_project_limits",
    json!({"project":"Kontur", "daily_budget":900000000}),
  )
  .expect("finite");
  assert_eq!(restored["limits"]["daily_budget"], 900000000);
  let refused = call(
    &page,
    "update_project_limits",
    json!({"project":"Kontur", "daily_budget":0}),
  )
  .expect_err("zero");
  assert!(matches!(refused, ToolError::Refused { code, .. } if code == "invalid_limits"));
}
