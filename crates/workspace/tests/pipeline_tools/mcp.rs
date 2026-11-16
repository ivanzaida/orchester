//! `create_pipeline`, `save_pipeline` and `duplicate_pipeline` publish a version or refuse with every reason and store
//! nothing (PV2-29); what they need a person for is what saving and publishing asked of one together; `get_pipeline`
//! knows no draft; `save_pipeline_draft` and `publish_pipeline` are unknown tools.

use serde_json::{Value, json};

use super::{Desktop, reasons, step};

/// `create_pipeline` of `name` from nothing but `steps`.
fn create(name: &str, steps: &Value) -> Value {
  json!({ "project": "Kontur", "name": name, "from": "empty", "steps": steps, "description": "Ships a change" })
}

/// Delivery, created as v1 with a plan step at an approval gate and an implement step after it.
fn delivery(desktop: &Desktop) -> Value {
  desktop.lets_automation_widen(true);
  let created = desktop.done(
    "create_pipeline",
    create(
      "Delivery",
      &json!([gated_plan(), step("implement", "Builder", &["plan"])]),
    ),
  );
  desktop.lets_automation_widen(false);
  created
}

/// A plan step on Builder that waits for a person's approval.
fn gated_plan() -> Value {
  let mut plan = step("plan", "Builder", &[]);
  plan["approval_gate"] = json!(true);
  plan
}

#[test]
fn pipeline_tools_create_publishes_v1_from_the_steps_given() {
  let desktop = Desktop::new();
  let created = delivery(&desktop)["pipeline"].clone();
  assert_eq!(created["published_version"], 1, "{created}");
  assert_eq!(created["description"], "Ships a change");
  assert_eq!(created["steps"][1]["runs_after"], json!(["plan"]));
  assert_eq!(created["steps"][0]["approval_gate"], true);
  assert!(!created.to_string().contains("draft"), "no draft state: {created}");
  assert_eq!(desktop.pipelines(), vec![("Delivery".to_owned(), 1, false)]);
  let read = desktop.done("get_pipeline", json!({ "project": "Kontur", "pipeline": "Delivery" }));
  assert_eq!(
    read["pipeline"], created,
    "get_pipeline reads what create_pipeline published"
  );
}

#[test]
fn pipeline_tools_create_refuses_what_cannot_publish_and_stores_nothing() {
  let desktop = Desktop::new();
  desktop.lets_automation_widen(true);
  let empty = desktop.refused(
    "create_pipeline",
    json!({ "project": "Kontur", "name": "Nothing", "from": "empty" }),
  );
  assert_eq!(empty["code"], "not_publishable", "{empty}");
  assert_eq!(reasons(&empty), ["pipelines.problems.noSteps"]);
  let unassigned = desktop.refused(
    "create_pipeline",
    create(
      "Half",
      &json!([{ "key": "plan", "name": "Plan" }, { "key": "build", "name": "Build", "runs_after": ["plan"] }]),
    ),
  );
  assert_eq!(unassigned["code"], "not_publishable", "{unassigned}");
  assert_eq!(
    reasons(&unassigned),
    ["pipelines.problems.noProfile", "pipelines.problems.noProfile"],
    "every step without a profile is named"
  );
  let cycle = desktop.refused(
    "create_pipeline",
    create(
      "Loop",
      &json!([step("plan", "Builder", &["build"]), step("build", "Builder", &["plan"])]),
    ),
  );
  assert_eq!(cycle["code"], "invalid_pipeline", "{cycle}");
  let starter = desktop.refused(
    "create_pipeline",
    json!({ "project": "Kontur", "name": "Starter", "from": "starter", "steps": [step("plan", "Builder", &[])] }),
  );
  assert_eq!(starter["code"], "steps_not_taken", "{starter}");
  assert_eq!(desktop.pipelines(), Vec::new(), "nothing was created");
}

#[test]
fn pipeline_tools_create_is_a_persons_first_version() {
  let desktop = Desktop::new();
  let refused = desktop.refused(
    "create_pipeline",
    create("Delivery", &json!([step("plan", "Builder", &[])])),
  );
  assert_eq!(refused["code"], "person_only", "{refused}");
  assert!(
    reasons(&refused).contains(&"appMcp.widens.firstPipelineVersion".to_owned()),
    "{refused}"
  );
  assert_eq!(desktop.pipelines(), Vec::new(), "nothing was created");
}

#[test]
fn pipeline_tools_save_publishes_the_next_version_or_refuses_with_nothing_stored() {
  let desktop = Desktop::new();
  delivery(&desktop);
  let mut plan = gated_plan();
  plan["budget"] = json!(1000);
  let save = |steps: Value| json!({ "project": "Kontur", "pipeline": "Delivery", "steps": steps });
  let stricter = json!([plan.clone(), step("implement", "Builder", &["plan"])]);
  let saved = desktop.done("save_pipeline", save(stricter.clone()));
  assert_eq!(
    saved["pipeline"]["published_version"], 2,
    "a stricter save needs no person: {saved}"
  );
  assert!(!saved.to_string().contains("draft"), "no draft state: {saved}");
  let unchanged = desktop.refused("save_pipeline", save(stricter));
  assert_eq!(unchanged["code"], "unchanged", "{unchanged}");
  let unassigned = desktop.refused(
    "save_pipeline",
    save(json!([plan, { "key": "implement", "name": "implement", "runs_after": ["plan"] }])),
  );
  assert_eq!(unassigned["code"], "not_publishable", "{unassigned}");
  assert_eq!(reasons(&unassigned), ["pipelines.problems.noProfile"]);
  assert_eq!(
    desktop.pipelines(),
    vec![("Delivery".to_owned(), 2, false)],
    "the refusals stored nothing"
  );
}

#[test]
fn pipeline_tools_save_needs_a_person_for_what_saving_and_publishing_asked_together() {
  let desktop = Desktop::new();
  delivery(&desktop);
  // The plan step no longer waits for approval (what saving asked a person for) and implement runs on another
  // profile (what publishing asked a person for).
  let steps = json!([step("plan", "Builder", &[]), step("implement", "Reviewer", &["plan"])]);
  let arguments = json!({ "project": "Kontur", "pipeline": "Delivery", "steps": steps });
  let refused = desktop.refused("save_pipeline", arguments.clone());
  assert_eq!(refused["code"], "person_only", "{refused}");
  let said = reasons(&refused);
  assert_eq!(said[0], "appMcp.errors.personOnly");
  for reason in ["appMcp.widens.approvalGate", "appMcp.widens.pipelineStep"] {
    assert!(said.contains(&reason.to_owned()), "{reason} in {said:?}");
  }
  assert_eq!(desktop.pipelines(), vec![("Delivery".to_owned(), 1, false)]);
  desktop.lets_automation_widen(true);
  let saved = desktop.done("save_pipeline", arguments);
  assert_eq!(saved["pipeline"]["published_version"], 2, "{saved}");
}

#[test]
fn pipeline_tools_draft_era_names_are_unknown_tools() {
  let desktop = Desktop::new();
  delivery(&desktop);
  let tools: Vec<_> = orchester_workspace::app_mcp::setup::listed()
    .into_iter()
    .chain(orchester_workspace::app_mcp::listed())
    .collect();
  let names: Vec<String> = tools.iter().map(|tool| tool.name.to_string()).collect();
  for gone in ["save_pipeline_draft", "publish_pipeline"] {
    assert!(!names.iter().any(|name| name == gone), "{gone} is not listed");
    let called = desktop.tool(
      gone,
      json!({ "project": "Kontur", "pipeline": "Delivery", "steps": [] }),
    );
    assert!(called.is_none(), "{gone} is no tool of the app");
  }
  for named in ["create_pipeline", "save_pipeline", "get_pipeline", "duplicate_pipeline"] {
    assert!(names.iter().any(|name| name == named), "{named} is listed");
  }
  for tool in &tools {
    let described = tool.description.as_deref().unwrap_or_default();
    for gone in ["save_pipeline_draft", "publish_pipeline"] {
      assert!(!described.contains(gone), "{} names {gone}", tool.name);
    }
  }
}

#[test]
fn pipeline_tools_duplicate_publishes_v1_or_nothing() {
  let desktop = Desktop::new();
  delivery(&desktop);
  let copy = |name: &str, profiles: Value| json!({ "project": "Kontur", "pipeline": "Delivery", "name": name, "step_profiles": profiles });
  let widens = desktop.refused(
    "duplicate_pipeline",
    copy("Review delivery", json!({ "implement": "Reviewer" })),
  );
  assert_eq!(widens["code"], "person_only", "{widens}");
  let unknown = desktop.tool("duplicate_pipeline", copy("Other", json!({ "deploy": "Reviewer" })));
  assert!(
    matches!(unknown, Some(Err(_))),
    "a step the source does not have is refused: {unknown:?}"
  );
  let taken = desktop.refused("duplicate_pipeline", copy("delivery", json!({})));
  assert_eq!(taken["code"], "name_taken", "{taken}");
  assert_eq!(
    desktop.pipelines(),
    vec![("Delivery".to_owned(), 1, false)],
    "a refused copy left nothing behind"
  );
  let same = desktop.done("duplicate_pipeline", copy("Delivery copy", json!({})));
  assert!(!same.to_string().contains("draft"), "no draft state: {same}");
  desktop.lets_automation_widen(true);
  desktop.done(
    "duplicate_pipeline",
    copy("Review delivery", json!({ "implement": "Reviewer" })),
  );
  let mut stored = desktop.pipelines();
  stored.sort();
  assert_eq!(
    stored,
    vec![
      ("Delivery".to_owned(), 1, false),
      ("Delivery copy".to_owned(), 1, false),
      ("Review delivery".to_owned(), 1, false),
    ]
  );
}
