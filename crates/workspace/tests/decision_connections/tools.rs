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
async fn decision_connections_tools_save_check_list_edit_delete_and_refuse_hosted() {
  let (repo, _) = project();
  let workspace = Workspace::new(AppDb::open(&repo.path().join("app.db")).expect("application data"));
  workspace.open(repo.path()).expect("register project");
  // The global widening switch does not authorize a hosted decision connection.
  workspace
    .set_automation_changes_permissions(true)
    .expect("person turns switch on");
  let vault = SharedVault::with(Arc::new(Vault { available: false }));
  let services = app_mcp::Services::new(
    workspace.clone(),
    Runners::new(
      repo.path().join("no-runner"),
      tokio::runtime::Handle::current(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    ),
    Servers::new(workspace, vault.clone()),
    Programs::processes(),
    (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
  )
  .with_vault(vault);
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
