//! What automation organizes on a shared task goes to the server as automation's (ORC-58): a new draft's organization,
//! which the server would record as the person's on creation, follows as a change of the draft, and an edit carries
//! its actor.

use orchester_core::{
  id::IdempotencyKey,
  task::organization::{ChangeActor, Priority, TaskOrganization, WorkflowStatus},
};

use super::{
  server_support::TestServer,
  support::{details, shared},
};

#[test]
fn shared_automation_organizes_a_new_and_an_edited_task_as_its_own() {
  let server = TestServer::start();
  let alice = shared(&server);
  let project = alice.server().link().project;
  let blocked = TaskOrganization::new(
    WorkflowStatus::Backlog,
    Priority::High,
    Some("Waits for a lurq release".into()),
    Vec::new(),
    Vec::new(),
  )
  .expect("a reason");
  let created = alice
    .with_store(|store| {
      store.create_organized(
        IdempotencyKey::generate(),
        &details("Paste images"),
        &blocked,
        ChangeActor::Orchester,
      )
    })
    .expect("created");
  assert_eq!(created.organization(), &blocked, "the cache has the organization");
  let history = server.task_history(project, created.id());
  let actors: Vec<&str> = history
    .as_array()
    .expect("changes")
    .iter()
    .map(|change| change["actor"].as_str().expect("actor"))
    .collect();
  assert_eq!(
    actors,
    ["orchester"],
    "the new draft's organization is automation's: {history}"
  );

  let task = alice.tasks().remove(0);
  let edited = task.organization().clone().with_priority(Priority::Urgent);
  alice
    .with_store(|store| {
      store.update_organized(
        task.id(),
        task.revision(),
        task.details(),
        (task.organization(), &edited),
        ChangeActor::Orchester,
      )
    })
    .expect("edit")
    .saved()
    .expect("saved as read");
  let history = server.task_history(project, task.id());
  let last = history.as_array().and_then(|changes| changes.last()).expect("a change");
  assert_eq!(last["actor"], "orchester", "{history}");
}
