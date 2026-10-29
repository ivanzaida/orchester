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

/// Lets automation answer approvals, as the person does in Settings: removing or raising the daily budget needs it.
pub(super) fn consent(page: &Settings) {
  page
    .window
    .launch
    .workspace()
    .expect("the session started")
    .set_automation_answers_approvals(true)
    .expect("the approvals opt-in");
}

#[test]
fn project_settings_domain_limits_accept_null_daily_budget_and_preserve_it_when_omitted() {
  let page = Settings::new(false);
  consent(&page);
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

#[test]
fn project_settings_domain_qa_slots_save_read_back_and_refuse_outside_one_to_sixteen() {
  let page = Settings::new(false);
  let listed = call(&page, "get_project", json!({"project":"Kontur"})).expect("read");
  assert_eq!(listed["project"]["limits"]["qa_slots"], 2, "the default");
  let changed = call(
    &page,
    "update_project_limits",
    json!({"project":"Kontur", "qa_slots":5}),
  )
  .expect("QA slots");
  assert_eq!(changed["limits"]["qa_slots"], 5);
  assert_eq!(changed["limits"]["agent_slots"], 4, "the agents' count stays");
  let read = call(&page, "get_project", json!({"project":"Kontur"})).expect("readback");
  assert_eq!(read["project"]["limits"]["qa_slots"], 5);
  // Another limit left out keeps the QA slots.
  call(
    &page,
    "update_project_limits",
    json!({"project":"Kontur", "agent_slots":3}),
  )
  .expect("agents");
  let read = call(&page, "get_project", json!({"project":"Kontur"})).expect("readback");
  assert_eq!(read["project"]["limits"]["qa_slots"], 5);
  for count in [0, 17] {
    let refused = call(
      &page,
      "update_project_limits",
      json!({"project":"Kontur", "qa_slots":count}),
    )
    .expect_err("outside 1 to 16");
    assert!(
      matches!(&refused, ToolError::Refused { code, reasons }
        if code == "invalid_limits" && reasons.iter().map(|reason| reason.key).eq(["settings.limits.qaSlotsInvalid"])),
      "{refused:?}"
    );
  }
  let read = call(&page, "get_project", json!({"project":"Kontur"})).expect("readback");
  assert_eq!(read["project"]["limits"]["qa_slots"], 5, "a refusal saves nothing");
}
