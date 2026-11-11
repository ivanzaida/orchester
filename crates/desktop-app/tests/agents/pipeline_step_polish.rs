//! The open step's polish of PV2-17: retries that survive a change of where changes requested go, the Agent field
//! saying why a tool cannot run, a model override dropped, and said, when the step moves to another CLI, and a gone
//! profile or tool said the same way on its flow card and in the field; the CLI an override is read against through
//! choices whose CLI is not known and after a Restore. One press on a strip chip that shows only in
//! part opens it. `cargo test -p orchester-app --test agents pipeline_step_polish`.

use std::collections::BTreeMap;

use lurq::app::events::MouseButton;
use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::{ConnectionId, ResourceId, SecretId},
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, ApprovalMode, CommandAction, CommandProgram, DatabaseQuery, DeniedBehavior,
    Pipeline, PipelineEdit, Slug, StepKind, Tool, ToolAction,
  },
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_flow::open,
  pipeline_hub::setup,
  pipeline_step::{CANCEL, chain, keys, open_step, rect, route, save_until, step_route, stored},
  pipeline_step_agent::{FIELD, PICKER, agent, executor, field_has, item, overriding, profile_on, single},
  pipeline_versions::{RESTORE, open_list},
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

/// Opens the Agent field's picker and chooses the item named `name`.
fn choose(agents: &mut Agents, name: &str) {
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(0)));
  let place = (0..6)
    .find(|place| agents.exists(&item(*place)) && agents.text(&format!("{}-name", item(*place))) == name)
    .unwrap_or_else(|| panic!("{name} is listed"));
  agents.click(&item(place));
  agents.until("chosen", |s| {
    s.text("pipeline-hub-step-agent-name") == name && !s.exists(PICKER)
  });
}

/// Where the *Agent* label stands, from the window's top.
fn label_y(agents: &mut Agents) -> f32 {
  agents
    .window
    .tree
    .get_element_by_id_mut("pipeline-hub-step-agent-row-label")
    .and_then(|element| element.bounds())
    .expect("the label")
    .y
}

/// A pipeline whose only step runs Builder (Codex) with the model overridden by `gpt-override`, its step open, and
/// Writer, a profile on Claude Code.
fn overridden_on_codex() -> (Agents, ResourceId, orchester_core::resource::StepKey, ResourceId) {
  let (mut agents, _) = setup();
  let builder = agents.profile(true);
  let connection = claude(&agents);
  let writer = profile_on(&agents, "Writer", Some(connection));
  let pipeline = overriding(&agents, builder, "gpt-override");
  let plan = keys(&agents, pipeline)[0].clone();
  open_step(&mut agents, pipeline, &plan);
  (agents, pipeline, plan, writer)
}

const DROPPED: &str = "pipeline-hub-step-agent-dropped";

#[test]
fn pipeline_step_polish_a_profile_on_another_cli_drops_the_model_override_and_says_so_until_save() {
  let (mut agents, pipeline, plan, writer) = overridden_on_codex();
  assert!(!agents.exists(DROPPED));
  let label = label_y(&mut agents);

  choose(&mut agents, "Writer");
  agents.until("held", |s| s.exists(CANCEL));
  assert_eq!(agents.text(DROPPED), "Model override dropped: it was for Codex");
  assert!(
    !agents.text("pipeline-hub-step-agent-runs").contains("gpt-override"),
    "the step runs on the new profile's own model"
  );
  let moved = label_y(&mut agents) - label;
  assert!(moved.abs() < 0.5, "the Agent label keeps its place: it moved {moved}");

  // Away to the pipeline and back: the edit still has the override dropped, and the field still says so.
  open(&mut agents, pipeline);
  agents.until("the flow", |s| !s.exists(FIELD));
  open_step(&mut agents, pipeline, &plan);
  agents.until("said again", |s| s.exists(DROPPED));
  assert_eq!(agents.text(DROPPED), "Model override dropped: it was for Codex");

  save_until(&mut agents, "saved", |s| {
    matches!(
      executor(s, pipeline, 0),
      StepKind::Agent { profile: Some(id), model: None, .. } if id == writer
    )
  });
  agents.until("said until Save", |s| !s.exists(DROPPED));
}

#[test]
fn pipeline_step_polish_choosing_the_overrides_cli_again_takes_the_override_back() {
  // Builder is a name two Codex profiles have here; either takes the override back, being on its CLI.
  let (mut agents, ..) = overridden_on_codex();
  choose(&mut agents, "Writer");
  agents.until("dropped", |s| s.exists(DROPPED));

  choose(&mut agents, "Builder");
  agents.until("taken back", |s| !s.exists(DROPPED));
  assert!(
    agents
      .text("pipeline-hub-step-agent-runs")
      .ends_with("· model: gpt-override"),
    "{}",
    agents.text("pipeline-hub-step-agent-runs")
  );
}

#[test]
fn pipeline_step_polish_an_override_kept_on_a_profile_whose_cli_is_not_known_stays_in_view() {
  let (mut agents, _) = setup();
  let builder = agents.profile(true);
  profile_on(&agents, "Explorer", None);
  let pipeline = overriding(&agents, builder, "gpt-override");
  let plan = keys(&agents, pipeline)[0].clone();
  open_step(&mut agents, pipeline, &plan);
  choose(&mut agents, "Explorer");
  agents.until("held", |s| s.exists(CANCEL));
  assert!(!agents.exists(DROPPED), "kept, not dropped");
  assert_eq!(
    agents.text("pipeline-hub-step-agent-runs"),
    "Choose a connection · model: gpt-override",
    "what to do, and the override kept"
  );
}

#[test]
fn pipeline_step_polish_an_override_kept_through_an_unknown_cli_is_dropped_on_another_known_one() {
  let (mut agents, _) = setup();
  let builder = agents.profile(true);
  let connection = claude(&agents);
  profile_on(&agents, "Writer", Some(connection));
  profile_on(&agents, "Explorer", None);
  let pipeline = overriding(&agents, builder, "gpt-override");
  let plan = keys(&agents, pipeline)[0].clone();
  open_step(&mut agents, pipeline, &plan);
  choose(&mut agents, "Explorer");
  agents.until("held", |s| s.exists(CANCEL));
  assert!(!agents.exists(DROPPED), "kept: Explorer's CLI is not known");

  // The override was for Codex, the CLI the step started on; Writer runs on Claude Code.
  choose(&mut agents, "Writer");
  agents.until("dropped", |s| s.exists(DROPPED));
  assert_eq!(agents.text(DROPPED), "Model override dropped: it was for Codex");
  assert!(
    !agents.text("pipeline-hub-step-agent-runs").contains("gpt-override"),
    "{}",
    agents.text("pipeline-hub-step-agent-runs")
  );
}

#[test]
fn pipeline_step_polish_a_gone_profile_reads_the_same_on_the_card_and_offers_to_choose_another() {
  let (mut agents, _) = setup();
  let retired = profile_on(&agents, "Retired", Some(agents.listed().connections[0].id()));
  let pipeline = single(&agents, agent(retired, None));
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(retired, Timestamp::now())
    .expect("archived");
  let plan = keys(&agents, pipeline)[0].clone();
  open(&mut agents, pipeline);
  let card = format!("pipeline-hub-flow-step-{plan}");
  agents.until("the card", |s| s.exists(&card));
  let said = agents.text(&format!("{card}-runs"));
  assert_eq!(said, "Profile no longer available");
  assert_eq!(agents.text(&format!("{card}-choose-words")), "Choose another");
  assert!(
    !agents.exists(&format!("{card}-fix")),
    "no Open profile: there is no profile to open"
  );

  open_step(&mut agents, pipeline, &plan);
  assert_eq!(
    agents.text("pipeline-hub-step-agent-name"),
    said,
    "the field names it as the card does"
  );
}

#[test]
fn pipeline_step_polish_a_tool_with_only_a_draft_is_named_and_said_to_be_unpublished_not_gone() {
  let (mut agents, _) = setup();
  let body = Tool::new(
    "",
    ToolAction::Command(CommandAction::new(
      CommandProgram::new("cargo").expect("program"),
      Vec::new(),
    )),
    None,
  )
  .expect("tool");
  let tool = ProjectDb::open(&agents.root)
    .expect("database")
    .create_resource(
      Slug::new("lint").expect("slug"),
      Name::new("Lint").expect("name"),
      &body,
      Timestamp::now(),
    )
    .expect("the tool")
    .id();
  let pipeline = single(&agents, StepKind::Tool { tool: Some(tool) });
  let plan = keys(&agents, pipeline)[0].clone();
  open(&mut agents, pipeline);
  let card = format!("pipeline-hub-flow-step-{plan}");
  agents.until("the card", |s| s.exists(&card));
  assert_eq!(agents.text(&format!("{card}-runs")), "Tool not published yet");
  assert!(
    agents.exists(&format!("{card}-fix")),
    "Open tool, where it is published"
  );

  open_step(&mut agents, pipeline, &plan);
  assert_eq!(agents.text("pipeline-hub-step-agent-name"), "Lint");
  assert_eq!(
    agents.text("pipeline-hub-step-agent-runs"),
    "This tool has no published version yet"
  );
}

#[test]
fn pipeline_step_polish_one_press_on_a_chip_partly_under_the_strips_arrow_opens_it_and_brings_it_into_view() {
  let mut agents = Agents::new(true);
  let long = chain(&agents, 16);
  let all = keys(&agents, long);
  open_step(&mut agents, long, &all[15]);
  agents.until("scrolled to the end", |s| {
    s.exists("pipeline-hub-step-strip-before-panel")
      && !s.exists("pipeline-hub-step-strip-after")
      && rect(s, "pipeline-hub-step-strip-before-panel").width > 0.0
  });
  agents.window.pass();
  let panel = rect(&mut agents, "pipeline-hub-step-strip-before-panel");
  let edge = panel.x + panel.width;
  let target = all
    .iter()
    .map(|key| (key.clone(), rect(&mut agents, &format!("pipeline-hub-step-chip-{key}"))))
    .find(|(_, chip)| chip.x < edge && chip.x + chip.width > edge + 4.0)
    .map(|(key, _)| key)
    .expect("a chip partly under ‹");

  // One press on what shows of it, as a person presses it.
  let scale = agents.window.tree.scale_factor();
  let middle = agents
    .window
    .tree
    .get_element_by_id_mut(&format!("pipeline-hub-step-chip-{target}"))
    .and_then(|element| element.bounds())
    .map(|bounds| bounds.y + bounds.height / 2.0)
    .expect("the chip");
  let (x, y) = ((edge + 2.0) * scale, middle * scale);
  agents.window.tree.mouse_move(x, y);
  agents.window.tree.mouse_down(x, y, MouseButton::Left);
  agents.window.tree.mouse_up(x, y, MouseButton::Left);
  agents.until("its step", |s| route(s) == step_route(long, &target));
  let id = format!("pipeline-hub-step-chip-{target}");
  agents.until("in view, clear of ‹", |s| {
    let chip = rect(s, &id);
    !s.exists("pipeline-hub-step-strip-before-panel") || {
      let panel = rect(s, "pipeline-hub-step-strip-before-panel");
      chip.x >= panel.x + panel.width
    }
  });
}

#[test]
fn pipeline_step_polish_unknown_codex_unknown_claude_drops_the_override_even_after_leaving_the_step() {
  let (mut agents, _) = setup();
  agents.profile(true);
  let connection = claude(&agents);
  profile_on(&agents, "Writer", Some(connection));
  let explorer = profile_on(&agents, "Explorer", None);
  let pipeline = overriding(&agents, explorer, "some-override");
  let plan = keys(&agents, pipeline)[0].clone();
  open_step(&mut agents, pipeline, &plan);

  // Codex is the first CLI known: there is nothing to drop the override on.
  choose(&mut agents, "Builder");
  assert!(!agents.exists(DROPPED), "kept on the first known CLI");
  choose(&mut agents, "Explorer");
  assert!(!agents.exists(DROPPED), "kept: Explorer's CLI is not known");

  // Away to the pipeline and back: the edit remembers that the step last ran on Codex.
  open(&mut agents, pipeline);
  agents.until("the flow", |s| !s.exists(FIELD));
  open_step(&mut agents, pipeline, &plan);

  choose(&mut agents, "Writer");
  agents.until("dropped on Claude Code", |s| s.exists(DROPPED));
  assert_eq!(agents.text(DROPPED), "Model override dropped");
  assert!(
    !agents.text("pipeline-hub-step-agent-runs").contains("some-override"),
    "{}",
    agents.text("pipeline-hub-step-agent-runs")
  );
}

#[test]
fn pipeline_step_polish_an_override_another_cli_set_goes_and_the_one_the_step_started_with_comes_back() {
  let (mut agents, _) = setup();
  let builder = agents.profile(true);
  let connection = claude(&agents);
  let writer = profile_on(&agents, "Writer", Some(connection));
  // v1 runs Writer (Claude Code) with an override of its own; v2, the one the edit starts from, Builder (Codex).
  let pipeline = overriding(&agents, writer, "claude-override");
  let plan = keys(&agents, pipeline)[0].clone();
  {
    let mut db = ProjectDb::open(&agents.root).expect("database");
    let latest = db
      .latest_version::<Pipeline>(pipeline)
      .expect("read")
      .expect("published");
    let mut edit = PipelineEdit::new(latest.body().clone());
    edit.update(&plan, |step| step.kind = agent(builder, Some("gpt-override")));
    let body = edit.validate().expect("a pipeline");
    db.publish_pipeline_edit(pipeline, &body, latest.number(), None, Timestamp::now())
      .expect("v2");
  }
  open(&mut agents, pipeline);
  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("v1", |s| s.exists(RESTORE));
  agents.click(RESTORE);
  agents.until("the edit holds v1", |s| !s.exists("pipeline-hub-version-viewing"));
  open_step(&mut agents, pipeline, &plan);
  assert!(
    agents
      .text("pipeline-hub-step-agent-runs")
      .ends_with("· model: claude-override")
  );

  // Claude Code's override is not taken onto Codex; the step's own Codex override comes back.
  choose(&mut agents, "Builder");
  agents.until("the started override", |s| {
    s.text("pipeline-hub-step-agent-runs")
      .ends_with("· model: gpt-override")
  });
  assert!(!agents.exists(DROPPED), "the step runs what it started with");
}
