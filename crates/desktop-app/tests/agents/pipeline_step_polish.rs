//! The open step's polish of PV2-17: retries that survive a change of where changes requested go, the Agent field
//! saying why a tool cannot run, and a model override dropped, and said, when the step moves to another CLI.
//! `cargo test -p orchester-app --test agents pipeline_step_polish`.

use std::collections::BTreeMap;

use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::{ConnectionId, ResourceId, SecretId},
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, ApprovalMode, DatabaseQuery, DeniedBehavior, Slug, StepKind, Tool, ToolAction,
  },
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::setup,
  pipeline_step::{CANCEL, chain, keys, open_step, save_until, stored},
  pipeline_step_agent::{FIELD, PICKER, executor, field_has, item, overriding, profile_on},
};

/// Whether the retries box is there and holds `text`.
fn retries_are(agents: &mut Agents, text: &str) -> bool {
  agents
    .window
    .tree
    .get_element_by_id_mut("pipeline-hub-step-retries")
    .and_then(|element| element.as_text_input())
    .is_some_and(|input| input.value() == text)
}

/// Chooses `label` in *When changes are requested* of step `key`, whose select is mounted anew after each save.
fn route_to(agents: &mut Agents, key: &orchester_core::resource::StepKey, label: &str) {
  let id = (0..9)
    .map(|epoch| format!("pipeline-hub-step-changes-{key}-{epoch}"))
    .find(|id| agents.exists(id))
    .expect("the select");
  let mut select = agents
    .window
    .tree
    .get_element_by_id_mut(&id)
    .and_then(|element| element.as_select())
    .expect("a select");
  let index = select
    .labels()
    .iter()
    .position(|option| &**option == label)
    .expect("an option");
  select.commit(index);
  agents.window.pass();
}

#[test]
fn pipeline_step_polish_changing_only_the_target_keeps_the_retries() {
  let mut agents = Agents::new(true);
  let ready = chain(&agents, 3);
  let build = keys(&agents, ready)[2].clone();
  open_step(&mut agents, ready, &build);
  route_to(&mut agents, &build, "01 Review stage 1");
  agents.until("one retry", |s| retries_are(s, "1"));
  agents.set("pipeline-hub-step-retries", "3");

  route_to(&mut agents, &build, "02 Review stage 2");
  agents.window.pass();
  assert!(retries_are(&mut agents, "3"), "the retries stay with the new target");
  let second = keys(&agents, ready)[1].clone();
  save_until(&mut agents, "the new target stored", |s| {
    stored(s, ready)
      .step(&build)
      .expect("step")
      .on_failure
      .as_ref()
      .is_some_and(|route| route.return_to == second)
  });
  let route = stored(&agents, ready).step(&build).expect("step").on_failure.clone();
  assert_eq!(route.map(|route| route.max_retries.get()), Some(3));
}

#[test]
fn pipeline_step_polish_unfinished_retries_text_is_kept_on_a_target_change() {
  let mut agents = Agents::new(true);
  let ready = chain(&agents, 3);
  let build = keys(&agents, ready)[2].clone();
  open_step(&mut agents, ready, &build);
  route_to(&mut agents, &build, "01 Review stage 1");
  agents.until("one retry", |s| retries_are(s, "1"));
  agents.set("pipeline-hub-step-retries", "many");
  agents.until("the bar says what to fix", |s| {
    s.text("team-hub-actions-hint").starts_with("Fix Retries")
  });

  route_to(&mut agents, &build, "02 Review stage 2");
  agents.window.pass();
  assert!(retries_are(&mut agents, "many"), "what was typed stays to be fixed");
  assert!(agents.text("team-hub-actions-hint").starts_with("Fix Retries"));
}

/// A published database tool, which a pipeline step cannot run in this version.
fn database_tool(agents: &Agents) -> ResourceId {
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    Name::new("QA").expect("name"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("a policy");
  let tool = Tool::new(
    "",
    ToolAction::DatabaseQuery(DatabaseQuery::new(SecretId::generate(), "SELECT 1".into()).expect("a query")),
    Some(policy),
  )
  .expect("a tool");
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = db
    .create_resource(
      Slug::new("orders-db").expect("slug"),
      Name::new("Orders database").expect("name"),
      &tool,
      Timestamp::now(),
    )
    .expect("the tool")
    .id();
  db.publish_draft::<Tool>(id, Timestamp::now()).expect("published");
  id
}

#[test]
fn pipeline_step_polish_the_agent_row_says_why_a_tool_cannot_run() {
  let (mut agents, [ready, ..]) = setup();
  database_tool(&agents);
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(1)));
  assert_eq!(agents.text(&format!("{}-name", item(1))), "Orders database");
  assert_eq!(
    agents.text(&format!("{}-runs", item(1))),
    "This tool can't run as a step yet"
  );
  agents.click(&item(1));
  agents.until("chosen", |s| {
    s.text("pipeline-hub-step-agent-name") == "Orders database"
  });
  assert_eq!(
    agents.text("pipeline-hub-step-agent-runs"),
    "This tool can't run as a step yet",
    "the field's own line says why"
  );
  assert!(field_has(&mut agents, "not-ready"));
}

/// A Claude Code connection of the project, besides its Codex one.
fn claude(agents: &Agents) -> ConnectionId {
  let connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Test Claude").expect("name"),
    AgentProvider::ClaudeCodeCli,
    agents.root.join("claude"),
    Timestamp::now(),
  )
  .expect("absolute program");
  let id = connection.id();
  ProjectDb::open(&agents.root)
    .expect("database")
    .create_connection(&connection)
    .expect("the connection");
  id
}

#[test]
fn pipeline_step_polish_a_profile_on_another_cli_drops_the_model_override_and_says_so_until_save() {
  let (mut agents, _) = setup();
  let builder = agents.profile(true);
  let connection = claude(&agents);
  let writer = profile_on(&agents, "Writer", Some(connection));
  let pipeline = overriding(&agents, builder, "gpt-override");
  let plan = keys(&agents, pipeline)[0].clone();
  open_step(&mut agents, pipeline, &plan);
  assert!(!agents.exists("pipeline-hub-step-agent-dropped"));

  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(2)));
  let place = (0..4)
    .find(|place| agents.exists(&item(*place)) && agents.text(&format!("{}-name", item(*place))) == "Writer")
    .expect("Writer is listed");
  agents.click(&item(place));
  agents.until("chosen", |s| {
    s.text("pipeline-hub-step-agent-name") == "Writer" && !s.exists(PICKER) && s.exists(CANCEL)
  });
  assert_eq!(
    agents.text("pipeline-hub-step-agent-dropped"),
    "Model override dropped: it was for Codex"
  );
  assert!(
    !agents.text("pipeline-hub-step-agent-runs").contains("gpt-override"),
    "the step runs on the new profile's own model"
  );

  save_until(&mut agents, "saved", |s| {
    matches!(
      executor(s, pipeline, 0),
      StepKind::Agent { profile: Some(id), model: None, .. } if id == writer
    )
  });
  agents.until("said until Save", |s| !s.exists("pipeline-hub-step-agent-dropped"));
}
