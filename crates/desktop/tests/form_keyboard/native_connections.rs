//! A native agent connection before native agents run (`docs/architecture/native-agent.md` §9, P1a):
//! wherever it appears it says it is not available yet and why, and nothing tests, edits or starts it.

use std::path::Path;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, ConnectionStatus, EndpointPreset},
  id::{ConnectionId, ResourceId, SecretId},
  name::Name,
  resource::{AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, Slug},
  store::{ConnectionStore, ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
  task_run::{agent_step, fill_new_task, has, init_repository, programs},
};

const INSPECTOR: &str = "connection-inspector";
const NOTICE: &str = "native-not-available";
const BLOCKED: &str = "agent-permissions-blocked";

/// A native Anthropic connection, a published Builder profile on it and a published pipeline whose
/// one step runs the Builder.
fn seed_native(repo: &Path) -> (ConnectionId, ResourceId) {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let now = Timestamp::now();
  let name = |value: &str| Name::new(value).expect("name");
  let endpoint = ApiEndpoint::new(
    EndpointPreset::Official,
    BaseUrl::new("https://api.anthropic.com").expect("base URL"),
    SecretId::generate(),
  );
  let connection = AgentConnection::native(
    ConnectionId::generate(),
    name("Claude API · Kontur"),
    AgentProvider::AnthropicApi,
    endpoint,
    now,
  )
  .expect("native connection")
  .into_connection();
  db.create_connection(&connection).expect("connection");
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Work carefully.").expect("prompt"),
    connection.id(),
    ModelId::new("claude-opus-5-5").expect("model"),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, now)
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(builder, now).expect("publish profile");
  let pipeline = Pipeline::new(vec![agent_step(
    builder,
    "implement",
    "Implement",
    "Append one line.",
    &[],
  )])
  .expect("pipeline");
  let id = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, now)
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(id, now).expect("publish pipeline");
  (connection.id(), builder)
}

fn seeded() -> (Desktop, ConnectionId, ResourceId) {
  let mut seeded = None;
  let desktop = Desktop::with_seeded_project(|repo| seeded = Some(seed_native(repo)));
  let (connection, builder) = seeded.expect("the project was seeded");
  (desktop, connection, builder)
}

#[test]
fn native_connection_reads_not_available_in_the_list_and_offers_no_test_or_edit() {
  let (mut desktop, connection, _) = seeded();
  desktop.wait_for("nav-connections");
  desktop.click("nav-connections");
  let row = format!("connection-{connection}");
  desktop.wait_for(&row);
  assert!(shows(&mut desktop, &row, "Not available yet"));
  assert!(shows(
    &mut desktop,
    &row,
    "Anthropic API (native) · Official preset · api.anthropic.com"
  ));
  assert!(shows(&mut desktop, &row, "Native"));
  desktop.wait_for(INSPECTOR);
  for text in [
    "Claude API · Kontur",
    "Not available yet",
    "Anthropic API (native)",
    "Endpoint",
    "https://api.anthropic.com",
    "Native agents are not available in this version yet.",
  ] {
    assert!(shows(&mut desktop, INSPECTOR, text), "{text}");
  }
  assert!(!has(&mut desktop, "refresh-models"));
  // Test and Edit are there but unavailable: neither checks nor opens anything.
  desktop.press("test-connection");
  desktop.press("edit-connection");
  desktop.pass();
  assert!(!has(&mut desktop, "connection-dialog"));
  let stored = ProjectDb::open(desktop.repo())
    .expect("open the project")
    .connection(connection)
    .expect("read")
    .expect("stored");
  assert_eq!(stored.status(), ConnectionStatus::NotChecked);
  assert!(stored.last_check().is_none());
}

#[test]
fn native_connection_reads_not_available_in_the_profile_and_the_pipeline_step() {
  let (mut desktop, _, builder) = seeded();
  desktop.wait_for("nav-agents");
  desktop.click("nav-agents");
  let item = format!("agent-{builder}");
  desktop.wait_for(&item);
  desktop.click(&item);
  desktop.wait_for(BLOCKED);
  assert!(shows(
    &mut desktop,
    BLOCKED,
    "This profile can’t run yet: its connection is a native agent"
  ));
  // One notice says it: no second notice and no model-source line repeat it.
  assert!(!has(&mut desktop, NOTICE));
  assert!(!has(&mut desktop, "agent-provider-explanation"));
  let labels = desktop
    .tree
    .get_element_by_id_mut("agent-connection-select")
    .and_then(|element| element.as_select())
    .expect("the connection select")
    .labels();
  assert!(
    labels
      .iter()
      .any(|label| &**label == "Claude API · Kontur · not available yet")
  );
  desktop.press("agent-tab-permissions");
  desktop.wait_for("agent-enforcement");
  assert!(shows(
    &mut desktop,
    "agent-enforcement",
    "Not enforced yet · native agents are not available"
  ));
  assert!(!shows(&mut desktop, "agent-enforcement", "Enforced by"));
  assert!(!shows(&mut desktop, "agent-enforcement", "known limits"));

  desktop.click("nav-pipelines");
  desktop.wait_for("pipeline-row");
  desktop.click("pipeline-row");
  desktop.wait_for("step-card-implement");
  desktop.press("step-card-implement");
  desktop.wait_for(NOTICE);
  assert!(shows(
    &mut desktop,
    NOTICE,
    "Native agents are not available in this version"
  ));
}

#[test]
fn native_connection_keeps_a_run_from_starting() {
  let (runner, _) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_native(repo);
  });
  fill_new_task(&mut desktop, "Append hello to README.md.", "200000");
  let problem = "Step “implement” runs on a native agent connection.";
  desktop.wait_until("the preflight problem", |desktop| {
    let shown = desktop.tree.root().is_some_and(|root| contains_text(root, problem));
    // Pressing before the runner is online fails visibly; pressing again retries the same run.
    if !shown && has(desktop, "create-and-run") {
      desktop.click("create-and-run");
    }
    shown
  });
  let db = ProjectDb::open(desktop.repo()).expect("open the project");
  for task in db.tasks().expect("tasks") {
    assert!(db.runs_of_task(task.id()).expect("runs").is_empty(), "no run started");
  }
}
