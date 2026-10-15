//! The Orchestrator's reads of a project's setup (`docs/design/orchestrator-toolset.md`, "Tools"): secrets by name
//! and use, whose values never reach a result; an instruction with its versions; skills as Skills lists and opens
//! them. Each with the link the app's router opens.

use std::sync::Arc;

use orchester_core::{
  name::Name,
  secret::{Environment, NewSecret, SecretKind, SecretName, SecretValue},
  store::SecretStore,
};
use orchester_store::ProjectDb;
use orchester_workspace::secrets::data::{add_in, replace_in};
use serde_json::{Value, json};

use super::{
  Served,
  reads::{every_link_opens, opens_in_the_app},
  vault::MemoryVault,
};

/// Values the vault holds; no result may hold them.
const SENTRY_VALUE: &str = "sntrys_VALUE-never-in-a-result-41f7";
const DEPLOY_VALUE: &str = "dpl_VALUE-never-in-a-result-9c2e";

fn named<'a>(listed: &'a Value, key: &str, name: &str) -> &'a Value {
  listed[key]
    .as_array()
    .and_then(|items| items.iter().find(|item| item["name"] == name))
    .unwrap_or_else(|| panic!("{name} among {key}: {listed}"))
}

#[test]
fn domain_reads_secrets_by_name_and_use_and_never_a_value() {
  // The served app's tools keep and read secret values in this vault, as the session hands its own to them.
  let vault = Arc::new(MemoryVault::default());
  let (mut served, seeded) = Served::seeded_on_vault("connections", vault.clone());
  let sentry = ProjectDb::open(&seeded.repo)
    .and_then(|db| db.secrets())
    .expect("the project's secrets")
    .into_iter()
    .find(|secret| secret.name().to_string() == "SENTRY_TOKEN")
    .expect("the seeded bearer secret");
  let value = |text: &str| SecretValue::new(text.to_owned()).expect("a secret value");
  replace_in(&seeded.repo, vault.as_ref(), sentry.id(), &value(SENTRY_VALUE)).expect("a value for SENTRY_TOKEN");
  let deploy = NewSecret {
    name: SecretName::new("DEPLOY_TOKEN").expect("a secret name"),
    kind: SecretKind::DeploymentToken,
    environment: Environment::Production,
  };
  add_in(&seeded.repo, vault.as_ref(), deploy, &value(DEPLOY_VALUE)).expect("a new secret");
  let kept: Vec<String> = vault.values().into_values().collect();
  assert!(kept.iter().any(|kept| kept == SENTRY_VALUE) && kept.iter().any(|kept| kept == DEPLOY_VALUE));

  let kontur = json!({ "project": "Kontur" });
  let listed = served.ok("list_secrets", &kontur);
  opens_in_the_app(&listed);
  let sentry = named(&listed, "secrets", "SENTRY_TOKEN");
  assert_eq!(sentry["environment"], "development", "{sentry}");
  assert_eq!(sentry["used_by"]["mcp_servers"][0]["name"], "sentry", "{sentry}");
  assert_eq!(sentry["link"], format!("/project/{}/secrets", seeded.project));
  let deploy = named(&listed, "secrets", "DEPLOY_TOKEN");
  assert_eq!(deploy["kind"], "deployment_token", "{deploy}");
  assert_eq!(deploy["environment"], "production", "{deploy}");
  let connections = served.ok("list_connections", &kontur);
  opens_in_the_app(&connections);
  let server = named(&connections, "mcp_servers", "sentry");
  let id = server["id"].as_str().expect("its id");
  assert_eq!(
    server["link"],
    format!("/project/{}/connections?selected={id}", seeded.project)
  );

  for (tool, arguments) in every_read(&mut served) {
    let text = match served.call(&tool, &arguments) {
      Ok(value) | Err(value) => value.to_string(),
    };
    assert!(
      !text.contains(SENTRY_VALUE) && !text.contains(DEPLOY_VALUE),
      "{tool} gave a secret value: {text}"
    );
  }
}

/// Every read-only tool the server lists, each with arguments that name what the `connections` scenario has:
/// with every project and with Kontur, and with each of its profiles, pipelines, tools and connections.
fn every_read(served: &mut Served) -> Vec<(String, Value)> {
  let kontur = json!({ "project": "Kontur" });
  let names = |served: &mut Served, tool: &str, key: &str, field: &str| -> Vec<Value> {
    let listed = served.ok(tool, &json!({ "project": "Kontur" }));
    listed[key]
      .as_array()
      .map_or_else(Vec::new, |items| items.iter().map(|item| item[field].clone()).collect())
  };
  let profiles = names(served, "list_profiles", "profiles", "id");
  let pipelines = names(served, "list_pipelines", "pipelines", "id");
  let tools = names(served, "list_tools", "tools", "id");
  let connections = names(served, "list_connections", "agents", "id");
  let with = |key: &str, values: &[Value]| -> Vec<Value> {
    values
      .iter()
      .map(|value| json!({ "project": "Kontur", key: value }))
      .collect()
  };
  let reads = served
    .tools()
    .into_iter()
    .filter(|tool| tool["annotations"]["readOnlyHint"] == true)
    .filter_map(|tool| tool["name"].as_str().map(str::to_owned));
  let mut calls = Vec::new();
  for tool in reads {
    let arguments = match tool.as_str() {
      "list_projects" | "list_accounts" => vec![json!({})],
      "get_profile" => with("profile", &profiles),
      "get_pipeline" => with("pipeline", &pipelines),
      "get_tool" => with("tool", &tools),
      "list_models" => with("connection", &connections),
      "get_task" => vec![json!({ "project": "Kontur", "key": "KON-1" })],
      "get_run" | "get_plan" | "get_changes" => vec![json!({ "project": "Kontur", "run": "1" })],
      "get_instruction" => vec![json!({ "project": "Kontur", "instruction": "any" })],
      "get_skill" => vec![json!({ "project": "Kontur", "skill": "any" })],
      name if name.starts_with("list_") => vec![json!({}), kontur.clone()],
      _ => vec![kontur.clone()],
    };
    calls.extend(arguments.into_iter().map(|arguments| (tool.clone(), arguments)));
  }
  assert!(
    calls.iter().any(|(tool, _)| tool == "list_secrets"),
    "the domain tools are listed: {calls:?}"
  );
  calls
}

#[test]
fn domain_reads_an_instruction_with_its_versions_and_skills_as_skills_shows_them() {
  let (mut served, seeded) = Served::seeded("empty");
  let rule = json!({
    "project": "Kontur",
    "name": "Tests first",
    "applies_in": "project",
    "agents": "all",
    "events": "start",
    "text": "Write the failing test first.",
  });
  served.ok("save_instruction_draft", &rule);
  served.ok(
    "publish_instruction",
    &json!({ "project": "Kontur", "instruction": "Tests first" }),
  );
  let got = served.ok(
    "get_instruction",
    &json!({ "project": "Kontur", "instruction": "tests first" }),
  );
  opens_in_the_app(&got);
  let instruction = &got["instruction"];
  assert_eq!(instruction["applies_in"]["kind"], "project", "{instruction}");
  assert_eq!(instruction["versions"][0]["version"], 1, "{instruction}");
  assert_eq!(
    instruction["versions"][0]["text"], "Write the failing test first.",
    "{instruction}"
  );
  assert_eq!(instruction["link"], format!("/project/{}/instructions", seeded.project));
  let missing = served.refused(
    "get_instruction",
    &json!({ "project": "Kontur", "instruction": "Nope" }),
  );
  assert_eq!(missing["code"], "instruction_not_found", "{missing}");

  let id = orchester_workspace::skills::data::create_in(&seeded.repo, Name::new("Release notes").expect("a name"))
    .expect("a skill");
  let skills = served.ok("list_skills", &json!({ "project": "Kontur" }));
  opens_in_the_app(&skills);
  let listed = named(&skills, "skills", "Release notes");
  assert_eq!(listed["state"]["state"], "draft", "{listed}");
  assert_eq!(listed["applies_in"]["kind"], "project", "{listed}");
  assert_eq!(listed["link"], format!("/project/{}/skills?skill={id}", seeded.project));
  let skill = served.ok("get_skill", &json!({ "project": "Kontur", "skill": "release notes" }));
  every_link_opens(&skill);
  let skill = &skill["skill"];
  assert_eq!(skill["id"], id.to_string(), "{skill}");
  assert_eq!(skill["files"], json!([]), "{skill}");
  assert_eq!(skill["versions"], json!([]), "{skill}");
  assert_eq!(skill["in_use_by"], json!([]), "{skill}");
  let missing = served.refused("get_skill", &json!({ "project": "Kontur", "skill": "Nope" }));
  assert_eq!(missing["code"], "skill_not_found", "{missing}");
}
