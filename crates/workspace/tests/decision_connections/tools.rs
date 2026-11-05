use std::sync::Arc;

use orchester_exec::programs::Programs;
use orchester_store::AppDb;
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, ToolError, Translate},
  runner::Runners,
  secrets::Vault as SharedVault,
  sharing::Servers,
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::Workspace,
};
use serde_json::{Value, json};

use super::{TYPESAFE, Vault, project, server};

async fn call(services: &app_mcp::Services, name: &str, arguments: Value) -> Result<Value, ToolError> {
  app_mcp::setup::call(
    services,
    &Translate::new(|message| message.key.to_owned()),
    name,
    arguments.as_object().expect("object").clone(),
  )
  .await
  .expect("setup tool")
}

#[tokio::test]
async fn mcp_decision_reviewer_connections_save_check_list_edit_delete_and_refuse_hosted() {
  let (repo, _) = project();
  let services = services(repo.path());
  let (url, task) = server("200 OK", TYPESAFE, "", false).await;
  let mut args = json!({"project":"Decision checks","name":"Decision","base_url":url,"key_secret":null});
  let saved = call(&services, "save_decision_connection", args.clone())
    .await
    .expect("add");
  let id = saved["connection"]["id"].as_str().expect("id");
  let checked = call(
    &services,
    "check_decision_connection",
    json!({"project":"Decision checks","connection":id}),
  )
  .await
  .expect("check");
  task.await.expect("server complete");
  assert_eq!(checked["connection"]["last_check"]["result"], "connected");
  let listed = call(&services, "list_connections", json!({"project":"Decision checks"}))
    .await
    .expect("list");
  assert_eq!(listed["decision_models"][0]["models"][0], "jev-latest");
  assert_models(&services, id).await;
  reject_key_values(&services, &args).await;
  args["connection"] = id.into();
  args["name"] = "Renamed".into();
  let renamed = call(&services, "save_decision_connection", args.clone())
    .await
    .expect("rename");
  assert_eq!(renamed["connection"]["last_check"], checked["connection"]["last_check"]);
  args["base_url"] = "https://hosted.example/v1".into();
  let error = call(&services, "save_decision_connection", args.clone())
    .await
    .expect_err("hosted is person only");
  assert!(matches!(error, ToolError::Refused { code, .. } if code == "person_only"));
  let mut hosted_add = args.clone();
  hosted_add.as_object_mut().expect("object").remove("connection");
  assert!(matches!(
    call(&services, "save_decision_connection", hosted_add).await,
    Err(ToolError::Refused { code, .. }) if code == "person_only"
  ));
  args["writer"] = "person".into();
  assert!(matches!(
    call(&services, "save_decision_connection", args).await,
    Err(ToolError::InvalidArguments(_))
  ));
  call(
    &services,
    "remove_decision_connection",
    json!({"project":"Decision checks","connection":id}),
  )
  .await
  .expect("delete");
  let listed = call(&services, "list_connections", json!({"project":"Decision checks"}))
    .await
    .expect("list");
  assert_eq!(listed["decision_models"], json!([]));
}

async fn assert_models(services: &app_mcp::Services, id: &str) {
  for named in [id, "decision", "Decision"] {
    let listed = call(
      services,
      "list_models",
      json!({"project":"Decision checks","connection":named}),
    )
    .await
    .expect("decision models by name or id");
    assert_eq!(listed["source"], "decision");
    assert_eq!(listed["checked"], true);
    assert_eq!(listed["models"], json!([{"id":"jev-latest","efforts":[]}]));
  }
}

async fn reject_key_values(services: &app_mcp::Services, args: &Value) {
  for field in ["key", "key_value", "api_key"] {
    let mut args = args.clone();
    args[field] = "synthetic-rejected-value".into();
    assert!(matches!(
      call(services, "save_decision_connection", args).await,
      Err(ToolError::InvalidArguments(_))
    ));
  }
}

#[test]
fn mcp_decision_reviewer_schemas_only_accept_secret_references_and_offer_no_test() {
  let tools: Vec<_> = app_mcp::listed().into_iter().chain(app_mcp::setup::listed()).collect();
  for name in [
    "save_decision_connection",
    "check_decision_connection",
    "remove_decision_connection",
    "set_reviewer",
  ] {
    let tool = tools.iter().find(|tool| tool.name == name).expect("listed tool");
    let properties = tool.input_schema["properties"].as_object().expect("properties");
    for field in ["key", "key_value", "api_key", "writer"] {
      assert!(!properties.contains_key(field), "{name} must not accept {field}");
    }
    assert_eq!(tool.input_schema["additionalProperties"], false);
  }
  assert!(
    !tools
      .iter()
      .any(|tool| tool.name.contains("decision") && tool.name.contains("test"))
  );
}

fn services(root: &std::path::Path) -> app_mcp::Services {
  let workspace = Workspace::new(AppDb::open(&root.join("app.db")).expect("application data"));
  workspace.open(root).expect("register project");
  // The global widening switch does not authorize a hosted decision connection.
  workspace
    .set_automation_changes_permissions(true)
    .expect("person turns switch on");
  let vault = SharedVault::with(Arc::new(Vault { available: false }));
  app_mcp::Services::new(
    workspace.clone(),
    Runners::new(
      root.join("no-runner"),
      tokio::runtime::Handle::current(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    ),
    Servers::new(
      workspace,
      vault.clone(),
      orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version"),
    ),
    Programs::processes(),
    (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
  )
  .with_vault(vault)
}

#[tokio::test]
async fn mcp_decision_reviewer_models_prioritize_ids_and_reject_ambiguous_names() {
  use orchester_core::{
    connection::{AgentConnection, AgentProvider},
    id::ConnectionId,
    name::Name,
    store::ConnectionStore,
    time::Timestamp,
  };
  use orchester_workspace::connections::decision::{Writer, save_in};

  let (repo, mut db) = project();
  let services = services(repo.path());
  let agent_id = ConnectionId::generate();
  let mut decision = super::draft("http://localhost:8000/v1");
  decision.name = Name::new(&agent_id.to_string()).expect("id as name");
  let decision_id = save_in(repo.path(), &decision, Writer::Automation).expect("decision");
  decision.id = Some(decision_id);
  let mut agent = AgentConnection::new(
    agent_id,
    Name::new(&decision_id.to_string()).expect("id as name"),
    AgentProvider::CodexCli,
    repo.path().join("codex"),
    Timestamp::now(),
  )
  .expect("agent");
  db.create_connection(&agent).expect("save agent");

  for (id, name, is_decision) in [
    (agent_id.to_string(), decision_id.to_string(), false),
    (decision_id.to_string(), agent_id.to_string(), true),
  ] {
    let listed = call(
      &services,
      "list_models",
      json!({"project":"Decision checks","connection":id.to_string()}),
    )
    .await
    .expect("exact id wins over another connection's name");
    assert_eq!(listed["connection"], name);
    assert_eq!(listed["source"] == "decision", is_decision);
  }

  decision.name = Name::new("ShÄrÉd").expect("name");
  save_in(repo.path(), &decision, Writer::Automation).expect("rename decision");
  agent.rename(Name::new("shäréd").expect("name"));
  db.save_connection(&agent).expect("rename agent");
  for named in ["ShÄrÉd", "shäréd", "SHÄRÉD"] {
    let error = call(
      &services,
      "list_models",
      json!({"project":"Decision checks","connection":named}),
    )
    .await
    .expect_err("shared name requires an id");
    assert!(matches!(error, ToolError::InvalidArguments(detail) if detail.contains("name it by id")));
  }
  for id in [agent_id.to_string(), decision_id.to_string()] {
    call(
      &services,
      "list_models",
      json!({"project":"Decision checks","connection":id.to_string()}),
    )
    .await
    .expect("shared names still resolve by id");
  }
}
