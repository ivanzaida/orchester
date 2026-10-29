//! `list_services`, `get_service`, `save_service_draft` and `publish_service`, and the same for stacks (ORC-269),
//! through Orchester's MCP server: a service runs a program, so making one, changing what it runs and its first
//! version wait for a person's switch, while its readiness and needs, and stacks, do not; publishing refuses a need
//! that is not published and a cycle of needs.
//! Run with `cargo test -p orchester-workspace --test service_tools service_tools`.

use std::{fs, sync::Arc};

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, ToolError, Translate},
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::Workspace,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// No secret is read or kept here.
struct NoVault;

impl SecretVault for NoVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::Failed("no secrets in these tests".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

/// The app's session with one local project, Kontur, whose tools are called as the app's MCP server calls them.
struct Desktop {
  workspace: Workspace,
  services: app_mcp::Services,
  runtime: Runtime,
  _repo: TempDir,
  _data: TempDir,
}

impl Desktop {
  fn new() -> Self {
    let repo = TempDir::new().expect("repository");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      Name::new("Kontur").expect("name"),
    );
    ProjectDb::create(repo.path(), project).expect("project");
    let data = TempDir::new().expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    workspace.open(repo.path()).expect("register the project");
    let runtime = Runtime::new().expect("runtime");
    let runners = Runners::new(
      repo.path().join("no-runner"),
      runtime.handle().clone(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    );
    let servers = Servers::new(workspace.clone(), Vault::with(Arc::new(NoVault)));
    let services = app_mcp::Services::new(
      workspace.clone(),
      runners,
      servers,
      Programs::processes(),
      (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
    );
    Self {
      workspace,
      services,
      runtime,
      _repo: repo,
      _data: data,
    }
  }

  fn call(&self, name: &str, args: &Value) -> Result<Value, ToolError> {
    let translate = Translate::new(|message| message.key.to_owned());
    let Value::Object(args) = args.clone() else {
      panic!("arguments are an object");
    };
    self
      .runtime
      .block_on(app_mcp::setup::call(&self.services, &translate, name, args))
      .expect("a setup tool")
  }

  fn ok(&self, name: &str, args: &Value) -> Value {
    self
      .call(name, args)
      .unwrap_or_else(|error| panic!("{name} refused: {}", json_of(&error)))
  }

  /// The error `name` is refused with: its code, message and reasons.
  fn refused(&self, name: &str, args: &Value) -> Value {
    match self.call(name, args) {
      Ok(value) => panic!("{name} did what it should refuse: {value}"),
      Err(error) => json_of(&error)["error"].clone(),
    }
  }

  fn let_automation_widen(&self, on: bool) {
    self
      .workspace
      .set_automation_changes_permissions(on)
      .expect("a person turns it on or off");
  }

  /// The services as list_services lists them, by name.
  fn listed(&self, tool: &str, key: &str) -> Vec<Value> {
    let listed = self.ok(tool, &json!({ "project": "Kontur" }));
    listed[key].as_array().expect("a list").clone()
  }
}

fn json_of(error: &ToolError) -> Value {
  error.to_json(|message| message.key.to_owned())
}

fn definition(name: &str, needs: &[&str]) -> Value {
  json!({
    "name": name,
    "program": "npm",
    "arguments": ["run", "dev", "--", "--port", "{{port}}"],
    "needs": needs,
    "readiness": { "kind": "port_open", "timeout_secs": 30 },
  })
}

fn save(service: Option<&str>, definition: Value) -> Value {
  let mut args = json!({ "project": "Kontur" });
  args["definition"] = definition;
  if let Some(service) = service {
    args["service"] = json!(service);
  }
  args
}

fn publish(service: &str) -> Value {
  json!({ "project": "Kontur", "service": service })
}

fn stack(stack: &str) -> Value {
  json!({ "project": "Kontur", "stack": stack })
}

#[test]
fn service_tools_make_and_publish_a_service_only_under_the_persons_switch() {
  let desktop = Desktop::new();
  let refused = desktop.refused("save_service_draft", &save(None, definition("api", &[])));
  assert_eq!(refused["code"], "person_only", "{refused}");
  assert_eq!(
    refused["reasons"],
    json!(["appMcp.errors.personOnly", "appMcp.services.widens.newService"])
  );
  assert!(
    desktop.listed("list_services", "services").is_empty(),
    "nothing was made"
  );

  desktop.let_automation_widen(true);
  let made = desktop.ok("save_service_draft", &save(None, definition("api", &[])));
  assert_eq!(made["service"]["state"]["state"], "draft", "{made}");
  assert_eq!(made["service"]["draft_kept"], true, "{made}");
  desktop.let_automation_widen(false);
  let first = desktop.refused("publish_service", &publish("api"));
  assert_eq!(first["code"], "person_only", "{first}");
  assert_eq!(
    first["reasons"],
    json!(["appMcp.errors.personOnly", "appMcp.services.widens.firstVersion"])
  );
  desktop.let_automation_widen(true);
  let published = desktop.ok("publish_service", &publish("api"));
  assert_eq!(published["service"]["state"]["published_version"], 1, "{published}");

  desktop.let_automation_widen(false);
  let mut ready = definition("api", &[]);
  ready["readiness"] = json!({ "kind": "http", "path": "/healthz", "timeout_secs": 60 });
  desktop.ok("save_service_draft", &save(Some("api"), ready.clone()));
  let next = desktop.ok("publish_service", &publish("API"));
  assert_eq!(
    next["service"]["state"]["published_version"], 2,
    "readiness starts nothing new: {next}"
  );

  let changes = [
    ("program", json!("node"), "appMcp.services.widens.program"),
    ("arguments", json!(["start"]), "appMcp.services.widens.arguments"),
    ("working_folder", json!("web"), "appMcp.services.widens.workingFolder"),
    (
      "variables",
      json!({ "TOKEN": { "secret": "API_TOKEN" } }),
      "appMcp.services.widens.variables",
    ),
    (
      "seed",
      json!({ "kind": "command", "program": "npm", "arguments": ["run", "seed"] }),
      "appMcp.services.widens.seed",
    ),
  ];
  for (field, value, reason) in changes {
    let mut changed = ready.clone();
    changed[field] = value;
    let refused = desktop.refused("save_service_draft", &save(Some("api"), changed));
    assert_eq!(refused["code"], "person_only", "{field}: {refused}");
    assert_eq!(refused["reasons"][1], reason, "{field}");
  }
  let read = desktop.ok("get_service", &publish("api"));
  assert_eq!(
    read["service"]["definition"]["program"], "npm",
    "nothing refused was saved"
  );
  assert_eq!(read["service"]["state"]["state"], "published", "{read}");
}

#[test]
fn service_tools_publish_refuses_a_missing_need_and_a_cycle() {
  let desktop = Desktop::new();
  desktop.let_automation_widen(true);
  desktop.ok("save_service_draft", &save(None, definition("web", &["api"])));
  let missing = desktop.refused("publish_service", &publish("web"));
  assert_eq!(missing["code"], "needs_not_published", "{missing}");
  assert_eq!(missing["reasons"], json!(["services.errors.needNotPublished"]));

  desktop.ok("save_service_draft", &save(None, definition("api", &[])));
  desktop.ok("publish_service", &publish("api"));
  desktop.ok("publish_service", &publish("web"));

  desktop.ok("save_service_draft", &save(Some("api"), definition("api", &["web"])));
  let cycle = desktop.refused("publish_service", &publish("api"));
  assert_eq!(cycle["code"], "needs_cycle", "{cycle}");
  assert_eq!(cycle["reasons"], json!(["services.errors.needsCycle"]));

  let listed = desktop.listed("list_services", "services");
  let api = listed
    .iter()
    .find(|service| service["name"] == "api")
    .expect("api is listed");
  assert_eq!(api["state"]["state"], "draft", "the refused draft stays: {api}");
  assert_eq!(api["needs"], json!(["web"]), "{api}");
  assert_eq!(api["readiness"], "port_open", "{api}");
  assert_eq!(api["ui_driver"], "none", "{api}");
}

#[test]
fn service_tools_save_a_definition_as_get_service_reads_it() {
  let desktop = Desktop::new();
  desktop.let_automation_widen(true);
  let full = json!({
    "name": "team-server",
    "program": "cargo",
    "arguments": ["run", "--", "--port", "{{port}}"],
    "working_folder": "server",
    "variables": { "RUST_LOG": { "plain": "info" }, "TEAM_TOKEN": { "secret": "TEAM_TOKEN" } },
    "needs": [],
    "readiness": { "kind": "log_line", "pattern": "listening on \\d+", "timeout_secs": 120 },
    "seed": { "kind": "requests", "requests": [{
      "method": "POST",
      "url": "{{url}}/api/teams",
      "body": "{\"name\":\"QA\"}",
      "headers": [{ "name": "Authorization", "value": { "secret": "TEAM_TOKEN" } }],
      "captures": [{ "name": "team_id", "path": "$.id" }],
    }] },
    "run_data": { "kind": "run_folder", "folder": "runs/{{run}}/team" },
    "ui_driver": "mcp",
    "owned_paths": ["server/**"],
  });
  desktop.ok("save_service_draft", &save(None, full.clone()));
  let read = desktop.ok("get_service", &publish("team-server"))["service"].clone();
  assert_eq!(read["definition"], full, "the definition reads back as saved");
  let revision = read["revision"].clone();

  let mut changed = read["definition"].clone();
  changed["owned_paths"] = json!(["server/**", "shared/**"]);
  let saved = desktop.ok(
    "save_service_draft",
    &json!({ "project": "Kontur", "service": "team-server", "definition": changed, "expected_revision": revision }),
  );
  assert_ne!(saved["service"]["revision"], revision, "a save makes a new revision");
  let stale = desktop.refused(
    "save_service_draft",
    &json!({ "project": "Kontur", "service": "team-server", "definition": full, "expected_revision": revision }),
  );
  assert_eq!(stale["code"], "draft_changed", "{stale}");

  let mut outside = definition("web", &[]);
  outside["working_folder"] = json!("../elsewhere");
  assert_eq!(
    desktop.refused("save_service_draft", &save(None, outside))["code"],
    "invalid_service"
  );
  assert_eq!(
    desktop.refused("save_service_draft", &save(None, definition("team-server", &[])))["code"],
    "name_taken"
  );
  assert_eq!(
    desktop.refused(
      "save_service_draft",
      &save(Some("team-server"), definition("renamed", &[]))
    )["code"],
    "name_not_editable"
  );
  assert_eq!(
    desktop.refused("get_service", &publish("nope"))["code"],
    "service_not_found"
  );
}

#[test]
fn service_tools_stacks_name_only_published_services() {
  let desktop = Desktop::new();
  desktop.let_automation_widen(true);
  desktop.ok("save_service_draft", &save(None, definition("web", &[])));
  desktop.ok("publish_service", &publish("web"));
  desktop.ok("save_service_draft", &save(None, definition("worker", &[])));
  desktop.let_automation_widen(false);

  let made = desktop.ok(
    "save_stack_draft",
    &json!({ "project": "Kontur", "name": "full", "services": ["web", "worker"] }),
  );
  assert_eq!(made["stack"]["services"], json!(["web", "worker"]), "{made}");
  assert_eq!(
    made["stack"]["state"]["state"], "draft",
    "a stack widens nothing: {made}"
  );
  let missing = desktop.refused("publish_stack", &stack("full"));
  assert_eq!(missing["code"], "needs_not_published", "{missing}");
  assert_eq!(missing["reasons"], json!(["services.errors.serviceNotPublished"]));

  desktop.ok(
    "save_stack_draft",
    &json!({ "project": "Kontur", "stack": "full", "services": ["web"] }),
  );
  let published = desktop.ok("publish_stack", &stack("full"));
  assert_eq!(published["stack"]["state"]["published_version"], 1, "{published}");
  let listed = desktop.listed("list_stacks", "stacks");
  assert_eq!(listed.len(), 1);
  assert_eq!(listed[0]["services"], json!(["web"]));
  assert!(desktop.ok("get_stack", &stack("full"))["stack"]["revision"].is_string());

  let renamed = json!({ "project": "Kontur", "stack": "full", "name": "all", "services": ["web"] });
  assert_eq!(
    desktop.refused("save_stack_draft", &renamed)["code"],
    "name_not_editable"
  );
  let empty = json!({ "project": "Kontur", "name": "none", "services": [] });
  assert_eq!(desktop.refused("save_stack_draft", &empty)["code"], "invalid_stack");
  let taken = json!({ "project": "Kontur", "name": "full", "services": ["web"] });
  assert_eq!(desktop.refused("save_stack_draft", &taken)["code"], "name_taken");
  assert_eq!(desktop.refused("publish_stack", &stack("full"))["code"], "no_draft");
}
