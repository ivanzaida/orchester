//! The domain tools the Orchestrator and the app's MCP server use, on a shared project: `create_module`,
//! `update_module`, and the module of `create_task` and `update_task` go to the server through the task writer.

use orchester_core::{module::ModuleState, task::organization::ChangeActor};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, ToolError, Translate},
  runner::Runners,
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
};
use serde_json::{Value, json};

use super::{
  oleh,
  server_support::TestServer,
  support::{Person, shared},
};

/// Calls the domain tool `name` with `args` as the app's MCP server does.
fn call(person: &Person, name: &str, args: Value) -> Result<Value, ToolError> {
  let runners = Runners::new(
    person.root().join("no-runner"),
    person.runtime.handle().clone(),
    Activity::new(),
    orchester_ipc::SessionEnd::Clean,
  );
  let services = app_mcp::Services::new(
    person.workspace.clone(),
    runners,
    person.servers.clone(),
    Programs::processes(),
    (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
  );
  let translate = Translate::new(|message| message.key.to_owned());
  let Value::Object(args) = args else {
    panic!("arguments are an object");
  };
  person
    .block_on(app_mcp::call(&services, &translate, name, args))
    .expect("a domain tool")
}

#[test]
fn shared_modules_the_domain_tools_write_through_the_server() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = oleh(&server, &alice);
  let project = alice.server().link().project;
  call(
    &alice,
    "create_module",
    json!({ "project": "Kontur", "name": "Checkout", "start_date": "2026-10-01" }),
  )
  .expect("create_module");
  let duplicate = call(
    &alice,
    "create_module",
    json!({ "project": "Kontur", "name": "checkout" }),
  );
  assert!(duplicate.is_err(), "the server's refusal: the name is taken");
  call(
    &alice,
    "update_module",
    json!({ "project": "Kontur", "module": "Checkout", "state": "closed" }),
  )
  .expect("update_module");
  let read = alice
    .block_on(oleh.client.modules(&oleh.token, project, 0))
    .expect("oleh reads the modules");
  assert_eq!(read.modules.len(), 1);
  assert_eq!(read.modules[0].fields.state, ModuleState::Closed);
  assert_eq!(read.modules[0].fields.start.as_deref(), Some("2026-10-01"));
  let module = read.modules[0].id;
  let created = call(
    &alice,
    "create_task",
    json!({ "project": "Kontur", "title": "Pay by card", "module": "Checkout", "story_points": 3 }),
  )
  .expect("create_task");
  let key = created["task"]["key"].as_str().expect("the new task's key").to_owned();
  let on_server = alice
    .block_on(oleh.client.tasks(&oleh.token, project))
    .expect("tasks")
    .tasks;
  let made = on_server.iter().find(|task| task.key == key).expect("on the server");
  assert_eq!(made.content.module, Some(module));
  call(
    &alice,
    "update_task",
    json!({ "project": "Kontur", "key": "ORC-1", "module": "Checkout" }),
  )
  .expect("update_task into the module");
  call(
    &alice,
    "update_task",
    json!({ "project": "Kontur", "key": key, "module": null }),
  )
  .expect("update_task out of the module");
  let read = alice
    .block_on(oleh.client.modules(&oleh.token, project, 0))
    .expect("the moves");
  let moves: Vec<_> = read
    .changes
    .iter()
    .map(|change| (change.from, change.to, change.actor))
    .collect();
  let orchester = ChangeActor::Orchester;
  assert_eq!(
    moves,
    [
      (None, Some(module), orchester),
      (None, Some(module), orchester),
      (Some(module), None, orchester)
    ],
    "the tools' moves are Orchester's"
  );
}
