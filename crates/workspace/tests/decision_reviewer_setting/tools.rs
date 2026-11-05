use orchester_exec::programs::Programs;
use orchester_store::AppDb;
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

use super::{connection, project, settings};

async fn call(services: &app_mcp::Services, name: &str, args: Value) -> Result<Value, ToolError> {
  app_mcp::call(
    services,
    &Translate::new(|m| m.key.to_owned()),
    name,
    args.as_object().expect("object").clone(),
  )
  .await
  .expect("tool")
}

fn services(repo: &std::path::Path) -> (app_mcp::Services, Workspace) {
  let workspace = Workspace::new(AppDb::open(&repo.join("app.db")).expect("app data"));
  workspace.open(repo).expect("register project");
  workspace.set_automation_answers_approvals(true).expect("approval gate");
  workspace
    .set_automation_changes_permissions(true)
    .expect("permissions gate");
  let vault = Vault::with(std::sync::Arc::new(NoVault));
  let services = app_mcp::Services::new(
    workspace.clone(),
    Runners::new(
      repo.join("no-runner"),
      tokio::runtime::Handle::current(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    ),
    Servers::new(workspace.clone(), vault.clone()),
    Programs::processes(),
    (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
  )
  .with_vault(vault);
  (services, workspace)
}

#[tokio::test]
async fn mcp_decision_reviewer_hosted_is_person_only_but_threshold_and_language_are_not() {
  let (repo, mut db) = project();
  let (services, workspace) = services(repo.path());
  let local = connection("http://localhost:8000/v1");
  let hosted = connection("https://review.example/v1");
  db.create_decision_connection(&local).expect("local");
  db.create_decision_connection(&hosted).expect("hosted");
  let local_args =
    json!({"project":"Reviewer","decision_connection":local.id().to_string(),"model":"jev-latest","threshold":0.85});
  let result = call(&services, "set_reviewer", local_args.clone())
    .await
    .expect("local reviewer");
  assert_eq!(result["approvals"]["reviewer"]["kind"], "decision");
  assert_eq!(result["approvals"]["reviewer"]["threshold"], 0.85);
  assert_eq!(result["approvals"]["reviewer"]["destination"]["host"], "localhost");
  let read = call(&services, "get_approvals", json!({"project":"Reviewer"}))
    .await
    .expect("read reviewer");
  assert_eq!(read, result);
  let mut args = local_args;
  args["decision_connection"] = hosted.id().to_string().into();
  assert!(matches!(
    call(&services, "set_reviewer", args.clone()).await,
    Err(ToolError::Refused { code, reasons })
      if code == "person_only" && reasons[0].key == "projectSettings.personOnly"
  ));
  db.save_auto_mode_settings(&settings(&hosted, "0.85"))
    .expect("person's choice");
  workspace
    .set_automation_changes_permissions(false)
    .expect("permissions off");
  for threshold in [0.90, 0.85, 0.10] {
    args["threshold"] = threshold.into();
    let result = call(&services, "set_reviewer", args.clone())
      .await
      .expect("threshold edit");
    assert_eq!(result["approvals"]["reviewer"]["threshold"], threshold);
  }
  args["threshold"] = 0.801.into();
  assert!(matches!(
    call(&services, "set_reviewer", args.clone()).await,
    Err(ToolError::InvalidArguments(_))
  ));
  args["threshold"] = 0.80.into();
  args["writer"] = "person".into();
  assert!(matches!(
    call(&services, "set_reviewer", args).await,
    Err(ToolError::InvalidArguments(_))
  ));
  let language = orchester_core::connection::AgentConnection::new(
    orchester_core::id::ConnectionId::generate(),
    orchester_core::name::Name::new("Claude").expect("name"),
    orchester_core::connection::AgentProvider::ClaudeCodeCli,
    std::env::current_exe().expect("program"),
    orchester_core::time::Timestamp::now(),
  )
  .expect("language");
  use orchester_core::store::ConnectionStore;
  db.create_connection(&language.into_connection()).expect("language");
  call(&services, "set_reviewer", json!({"project":"Reviewer"}))
    .await
    .expect("switch back with widening off");
}
#[tokio::test]
async fn mcp_decision_reviewer_gate_connection_and_model_changes() {
  let (repo, mut db) = project();
  let (services, workspace) = services(repo.path());
  let mut first = connection("http://localhost:8000/v1");
  let other_model = orchester_core::resource::ModelId::new("other-model").expect("model");
  first.record_models(vec![super::model(), other_model.clone()]);
  let mut second = connection("http://localhost:8001/v1");
  second.rename(orchester_core::name::Name::new("Second decision").expect("name"));
  db.create_decision_connection(&first).expect("first");
  db.create_decision_connection(&second).expect("second");
  let base = settings(&first, "0.80");
  db.save_auto_mode_settings(&base).expect("baseline");
  workspace
    .set_automation_changes_permissions(false)
    .expect("permissions off");
  for (connection, model) in [(second.id(), super::model()), (first.id(), other_model)] {
    let args = json!({
      "project": "Reviewer", "decision_connection": connection.to_string(), "model": model.as_str()
    });
    assert!(matches!(call(&services, "set_reviewer", args.clone()).await,
        Err(ToolError::Refused { code, .. }) if code == "person_only"));
    assert_eq!(db.auto_mode_settings().expect("unchanged"), base);
    workspace
      .set_automation_changes_permissions(true)
      .expect("permissions on");
    call(&services, "set_reviewer", args).await.expect("authorized change");
    db.save_auto_mode_settings(&base).expect("reset");
    workspace
      .set_automation_changes_permissions(false)
      .expect("permissions off");
  }
}
#[tokio::test]
async fn mcp_decision_reviewer_default_threshold_and_invalid_arguments() {
  let (repo, mut db) = project();
  let (services, _) = services(repo.path());
  let local = connection("http://localhost:8000/v1");
  db.create_decision_connection(&local).expect("local");
  let args = json!({"project":"Reviewer","decision_connection":local.id().to_string()});
  let result = call(&services, "set_reviewer", args.clone())
    .await
    .expect("default threshold");
  assert_eq!(result["approvals"]["reviewer"]["threshold"], 0.80);
  for threshold in [0.0, 1.0, -0.1, 0.801] {
    let mut invalid = args.clone();
    invalid["threshold"] = threshold.into();
    assert!(matches!(
      call(&services, "set_reviewer", invalid).await,
      Err(ToolError::InvalidArguments(_))
    ));
  }
  for field in ["connection", "effort", "key", "key_value"] {
    let mut invalid = args.clone();
    invalid[field] = "rejected".into();
    assert!(matches!(
      call(&services, "set_reviewer", invalid).await,
      Err(ToolError::InvalidArguments(_))
    ));
  }
}

struct NoVault;

impl orchester_core::secret::SecretVault for NoVault {
  fn store(
    &self,
    _: &orchester_core::secret::VaultKey,
    _: &orchester_core::secret::SecretValue,
  ) -> Result<(), orchester_core::secret::VaultError> {
    panic!("reviewer settings must not use a credential");
  }
  fn read(
    &self,
    _: &orchester_core::secret::VaultKey,
  ) -> Result<orchester_core::secret::SecretValue, orchester_core::secret::VaultError> {
    panic!("reviewer settings must not use a credential");
  }
  fn delete(&self, _: &orchester_core::secret::VaultKey) -> Result<(), orchester_core::secret::VaultError> {
    panic!("reviewer settings must not use a credential");
  }
}
