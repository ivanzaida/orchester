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

/// Whether `refused` is `invalid_limits` for exactly the reasons `keys`.
fn refused_for(refused: &ToolError, keys: &[&str]) -> bool {
  matches!(refused, ToolError::Refused { code, reasons }
    if code == "invalid_limits" && reasons.iter().map(|reason| reason.key).eq(keys.iter().copied()))
}

#[test]
fn project_settings_domain_service_slots_save_the_computers_limit_and_the_projects_share_and_read_them_back() {
  let page = Settings::new(false);
  let read = call(&page, "get_project", json!({"project":"Kontur"})).expect("read");
  assert_eq!(
    read["project"]["limits"]["service_slots"], 4,
    "the project's default share"
  );
  assert_eq!(
    read["project"]["limits"]["computer_service_slots"], 8,
    "the computer's default"
  );

  let changed = call(
    &page,
    "update_project_limits",
    json!({"project":"Kontur", "computer_service_slots":12, "service_slots":10}),
  )
  .expect("services at once");
  assert_eq!(changed["limits"]["computer_service_slots"], 12);
  assert_eq!(changed["limits"]["service_slots"], 10);
  assert_eq!(changed["limits"]["qa_slots"], 2, "the QA apps' count stays");
  let read = call(&page, "get_project", json!({"project":"Kontur"})).expect("readback");
  assert_eq!(read["project"]["limits"]["service_slots"], 10);
  assert_eq!(read["project"]["limits"]["computer_service_slots"], 12);
  let listed = call(&page, "list_projects", json!({})).expect("listed");
  let kontur = listed["projects"]
    .as_array()
    .expect("projects")
    .iter()
    .find(|project| project["name"] == "Kontur")
    .expect("Kontur");
  assert_eq!(kontur["limits"]["service_slots"], 10);
  assert_eq!(kontur["limits"]["computer_service_slots"], 12);
  assert_eq!(
    page
      .window
      .launch
      .workspace()
      .expect("the session")
      .service_slots()
      .expect("the computer's limit")
      .get()
      .get(),
    12,
    "the computer's limit is an application setting"
  );

  // Another limit left out keeps both.
  call(
    &page,
    "update_project_limits",
    json!({"project":"Kontur", "agent_slots":3}),
  )
  .expect("agents");
  let read = call(&page, "get_project", json!({"project":"Kontur"})).expect("readback");
  assert_eq!(read["project"]["limits"]["service_slots"], 10);
  assert_eq!(read["project"]["limits"]["computer_service_slots"], 12);
}

#[test]
fn project_settings_domain_service_slots_refuse_a_share_above_the_computers_limit_and_outside_one_to_32() {
  let page = Settings::new(false);
  call(
    &page,
    "update_project_limits",
    json!({"project":"Kontur", "computer_service_slots":12, "service_slots":10}),
  )
  .expect("services at once");
  // A share is at most the computer's limit, the one kept or the one given with it.
  let above = call(
    &page,
    "update_project_limits",
    json!({"project":"Kontur", "service_slots":13}),
  )
  .expect_err("above the computer's limit");
  assert!(
    refused_for(&above, &["settings.limits.serviceSlotsAboveComputer"]),
    "{above:?}"
  );
  let lowered = call(
    &page,
    "update_project_limits",
    json!({"project":"Kontur", "computer_service_slots":6, "service_slots":7}),
  )
  .expect_err("above the new limit");
  assert!(
    refused_for(&lowered, &["settings.limits.serviceSlotsAboveComputer"]),
    "{lowered:?}"
  );
  for (field, count) in [("service_slots", 0), ("computer_service_slots", 33)] {
    let refused = call(
      &page,
      "update_project_limits",
      json!({"project":"Kontur", field: count}),
    )
    .expect_err("outside 1 to 32");
    assert!(
      refused_for(&refused, &["settings.limits.serviceSlotsInvalid"]),
      "{refused:?}"
    );
  }
  let read = call(&page, "get_project", json!({"project":"Kontur"})).expect("readback");
  assert_eq!(
    read["project"]["limits"]["service_slots"], 10,
    "a refusal saves nothing"
  );
  assert_eq!(
    read["project"]["limits"]["computer_service_slots"], 12,
    "a refusal saves nothing"
  );
}
