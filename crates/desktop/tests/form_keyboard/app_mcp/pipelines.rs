//! Pipelines and the code index setting through Orchester's MCP server: a pipeline copied under a new
//! name with another profile on one agent step and published as v1, the refusals of a copy the app
//! would not make, and the code index turned off and on as Project settings does.

use std::path::Path;

use orchester_core::{
  code_index::CodeIndexSetting,
  resource::{AgentProfile, Pipeline, ResourceKind, Slug, StepKind},
  store::{ResourceStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{agent, ok, refused};
use crate::{
  harness::Desktop,
  task_run::{agent_step, seed_steps},
  tool_run::{seed_tool, tool_step},
};

/// Delivery: Implement and Review on the Builder, then the Checks tool; and a second profile,
/// Reviewer, published beside the Builder.
fn seeded() -> Desktop {
  Desktop::with_seeded_project_and_app_mcp(|repo| {
    let tool = seed_tool(repo, "orchester-test-command");
    seed_steps(repo, &repo.join("orchester-fake-codex"), |builder| {
      vec![
        agent_step(builder, "implement", "Implement", "Append one line.", &[]),
        agent_step(builder, "review", "Review", "Review the line.", &["implement"]),
        tool_step(tool),
      ]
    });
    seed_reviewer(repo);
  })
}

/// A published profile Reviewer, as the Builder is.
fn seed_reviewer(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let now = Timestamp::now();
  let builder = db.resources(ResourceKind::AgentProfile).expect("profiles")[0].id();
  let body = db
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("published")
    .body()
    .clone();
  let name = orchester_core::name::Name::new("Reviewer").expect("name");
  let reviewer = db
    .create_resource(Slug::new("reviewer").expect("slug"), name, &body, now)
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(reviewer, now).expect("publish");
}

fn copy(changes: &Value, name: &str) -> Value {
  json!({"project": "Kontur", "pipeline": "Delivery", "name": name, "step_profiles": changes})
}

#[test]
fn app_mcp_duplicates_a_pipeline_with_another_profile_and_publishes_it() {
  let desktop = seeded();
  let mut agent = agent(&desktop);
  let copied = ok(
    &mut agent,
    "duplicate_pipeline",
    &copy(&json!({"Review": "reviewer"}), "Delivery with a reviewer"),
  );
  let pipeline = &copied["pipeline"];
  assert_eq!(pipeline["name"], "Delivery with a reviewer");
  assert_eq!(pipeline["version"], 1);
  let names: Vec<&str> = pipeline["steps"]
    .as_array()
    .expect("steps")
    .iter()
    .filter_map(|step| step["name"].as_str())
    .collect();
  assert_eq!(names, ["Implement", "Review", "Checks"]);

  let db = ProjectDb::open(desktop.repo()).expect("open");
  let profile = |name: &str| {
    db.resources(ResourceKind::AgentProfile)
      .expect("profiles")
      .into_iter()
      .find(|resource| resource.name().as_str() == name)
      .expect("profile")
      .id()
  };
  let id = pipeline["id"].as_str().expect("an id").parse().expect("a pipeline id");
  let version = db.latest_version::<Pipeline>(id).expect("read").expect("published v1");
  let profiles: Vec<_> = version
    .body()
    .steps()
    .iter()
    .map(|step| match &step.kind {
      StepKind::Agent { profile, .. } => *profile,
      StepKind::Tool { .. } => None,
    })
    .collect();
  assert_eq!(profiles, [Some(profile("Builder")), Some(profile("Reviewer")), None]);
  // The source keeps its steps; a step is also named by its number.
  let listed = ok(&mut agent, "list_pipelines", &json!({"project": "Kontur"}));
  assert_eq!(listed["pipelines"].as_array().map(Vec::len), Some(2));
  let numbered = ok(
    &mut agent,
    "duplicate_pipeline",
    &copy(&json!({"1": "Reviewer"}), "Reviewed first"),
  );
  assert_eq!(numbered["pipeline"]["version"], 1);
}

#[test]
fn app_mcp_refuses_a_copy_the_app_would_not_make() {
  let desktop = seeded();
  let mut agent = agent(&desktop);
  for (changes, name, code) in [
    (json!({}), "Delivery", "name_taken"),
    (json!({}), "  ", "invalid_name"),
    (json!({"Deploy": "Reviewer"}), "Copy one", "step_not_found"),
    (json!({"9": "Reviewer"}), "Copy two", "step_not_found"),
    (json!({"Review": "Designer"}), "Copy three", "profile_not_found"),
    (json!({"Checks": "Reviewer"}), "Copy four", "not_an_agent_step"),
  ] {
    let error = refused(&mut agent, "duplicate_pipeline", &copy(&changes, name));
    assert_eq!(error["code"], code, "{changes} {name}: {error}");
  }
  let unknown = refused(
    &mut agent,
    "duplicate_pipeline",
    &json!({"project": "Kontur", "pipeline": "Release", "name": "Copy five"}),
  );
  assert_eq!(unknown["code"], "pipeline_not_found");
  let listed = ok(&mut agent, "list_pipelines", &json!({"project": "Kontur"}));
  assert_eq!(
    listed["pipelines"].as_array().map(Vec::len),
    Some(1),
    "nothing was created"
  );
}

#[test]
fn app_mcp_turns_the_code_index_off_and_on() {
  let desktop = seeded();
  let mut agent = agent(&desktop);
  let setting = || {
    ProjectDb::open(desktop.repo())
      .expect("open")
      .code_index()
      .expect("read")
  };
  let listed = ok(&mut agent, "list_projects", &json!({}));
  assert_eq!(
    listed["projects"][0]["code_index"], true,
    "on until a person turns it off"
  );
  let off = ok(
    &mut agent,
    "set_code_index",
    &json!({"project": "Kontur", "enabled": false}),
  );
  assert_eq!(off["code_index"], false);
  assert_eq!(setting(), CodeIndexSetting::Off);
  let listed = ok(&mut agent, "list_projects", &json!({}));
  assert_eq!(listed["projects"][0]["code_index"], false);
  let on = ok(
    &mut agent,
    "set_code_index",
    &json!({"project": "Kontur", "enabled": true}),
  );
  assert_eq!(on["code_index"], true);
  assert_eq!(setting(), CodeIndexSetting::On);
  let error = refused(&mut agent, "set_code_index", &json!({"project": "Kontur"}));
  assert_eq!(error["code"], "invalid_arguments");
}
