//! The step's *Agent* field and the Agent picker under it (`Pipelines · v2` P2 `u2rYEH`, P10 `HjgZ8`, K4 `CwLfL`;
//! PV2-08): the profile's name and what it runs on, the picker opening under the field from its start, choosing a
//! profile or a tool (also one that is not ready) changing the edit that *Cancel* puts back, a model the step overrides
//! kept and said, and the foot's two ways out. `cargo test -p orchester-app --test agents pipeline_step_agent`.

use std::collections::BTreeMap;

use lurq::app::events::MouseButton;
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  connection::mcp::{McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, StdioServer},
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, CommandAction, CommandProgram, ModelId, Pipeline, PipelineEdit, PromptText, ProviderSettings,
    RepositoryAccess, Slug, StepKind, Tool, ToolAction,
  },
  store::{McpServerStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::setup,
  pipeline_hub_menu::key,
  pipeline_step::{CANCEL, keys, open_step, route, save_until, stored},
};

pub(super) const FIELD: &str = "pipeline-hub-step-agent";
pub(super) const PICKER: &str = "pipeline-hub-step-agent-picker";

pub(super) fn item(place: usize) -> String {
  format!("{PICKER}-item-{place}")
}

/// What the step at `place` of the stored pipeline runs.
pub(super) fn executor(agents: &Agents, pipeline: ResourceId, place: usize) -> StepKind {
  stored(agents, pipeline).steps()[place].kind.clone()
}

/// A command tool `Checks` that runs `cargo`, published.
pub(super) fn checks(agents: &Agents) -> ResourceId {
  let body = Tool::new(
    "",
    ToolAction::Command(CommandAction::new(
      CommandProgram::new("cargo").expect("program"),
      Vec::new(),
    )),
    None,
  )
  .expect("tool");
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = db
    .create_resource(
      Slug::new("checks").expect("slug"),
      Name::new("Checks").expect("name"),
      &body,
      Timestamp::now(),
    )
    .expect("the tool")
    .id();
  db.publish_draft::<Tool>(id, Timestamp::now()).expect("published");
  id
}

/// A published profile `name` on `connection` (`None`: one the project does not have, as when the connection is removed
/// after the profile was published).
pub(super) fn profile_on(agents: &Agents, name: &str, connection: Option<ConnectionId>) -> ResourceId {
  let profile = AgentProfile::new(
    Name::new(name).expect("name"),
    String::new(),
    PromptText::new("Do the work.").expect("instructions"),
    connection.unwrap_or_else(ConnectionId::generate),
    ModelId::new("test-model").expect("model"),
    ProviderSettings::default(),
  );
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = db
    .create_resource(
      Slug::from_name(name).expect("slug"),
      Name::new(name).expect("name"),
      &profile,
      Timestamp::now(),
    )
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(id, Timestamp::now())
    .expect("published");
  id
}

/// A published pipeline whose only step runs `profile` and overrides the model with `model`.
pub(super) fn overriding(agents: &Agents, profile: ResourceId, model: &str) -> ResourceId {
  single(agents, agent(profile, Some(model)))
}

pub(super) fn agent(profile: ResourceId, model: Option<&str>) -> StepKind {
  StepKind::Agent {
    profile: Some(profile),
    repository: RepositoryAccess::ReadOnly,
    model: model.map(|model| ModelId::new(model).expect("model")),
    permissions: StepPermissions::default(),
  }
}

/// A published pipeline whose only step, *Plan*, is `kind`.
pub(super) fn single(agents: &Agents, kind: StepKind) -> ResourceId {
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let key = edit.add_step(Name::new("Plan").expect("name"), None);
  edit.update(&key, |step| step.kind = kind);
  let body = edit.validate().expect("a pipeline");
  ProjectDb::open(&agents.root)
    .expect("database")
    .create_published(
      Slug::new("single").expect("slug"),
      Name::new("Single").expect("name"),
      &body,
      Timestamp::now(),
    )
    .expect("the pipeline")
    .0
    .id()
}

/// The first step of the ready pipeline open.
fn first_step() -> (Agents, [ResourceId; 3]) {
  let (mut agents, ids) = setup();
  let plan = keys(&agents, ids[0])[0].clone();
  open_step(&mut agents, ids[0], &plan);
  (agents, ids)
}

#[test]
fn the_field_shows_the_profile_and_what_it_runs_on_and_the_step_has_no_model_field() {
  let (mut agents, _) = first_step();
  assert_eq!(agents.text("pipeline-hub-step-agent-name"), "Builder");
  let connection = agents.listed().connections[0].name().as_str().to_owned();
  let runs = agents.text("pipeline-hub-step-agent-runs");
  assert!(
    runs.contains(&connection) && runs.ends_with("· test-model"),
    "the CLI, the connection and the model: {runs}"
  );
  assert!(
    !agents.exists("pipeline-hub-step-model"),
    "the model is the profile's, not the step's"
  );
  assert!(!agents.exists(&item(0)), "the picker is closed");
}

#[test]
fn the_picker_opens_under_the_field_from_its_start_marks_the_chosen_and_esc_closes_it() {
  let (mut agents, _) = first_step();
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(PICKER));
  let (field, picker) = (rect(&mut agents, FIELD), rect(&mut agents, PICKER));
  assert!(
    (picker.0 - field.0).abs() < 1.0,
    "left-aligned with the field: {field:?} {picker:?}"
  );
  assert!(picker.1 >= field.1 + field.3, "under the field: {field:?} {picker:?}");
  assert_eq!(rect(&mut agents, PICKER).2.round(), 320.0, "320 wide");
  assert_eq!(agents.text(&format!("{}-name", item(0))), "Builder");
  assert!(
    agents.exists(&format!("{PICKER}-group-profiles")),
    "the profiles are grouped"
  );
  key(&mut agents, "Escape");
  agents.until("closed", |s| !s.exists(PICKER));
}

/// x, y, width, height of `id`.
fn rect(agents: &mut Agents, id: &str) -> (f32, f32, f32, f32) {
  let bounds = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (bounds.x, bounds.y, bounds.width, bounds.height)
}

#[test]
fn choosing_a_tool_runs_the_step_with_it_at_once_and_cancel_puts_the_profile_back() {
  let (mut agents, [ready, ..]) = setup();
  checks(&agents);
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(1)));
  assert_eq!(
    agents.text(&format!("{PICKER}-group-tools")),
    "Tools",
    "a tool is in its own group"
  );
  assert_eq!(agents.text(&format!("{}-name", item(1))), "Checks");
  assert_eq!(
    agents.text(&format!("{}-runs", item(1))),
    "cargo · its exit code decides the step"
  );
  agents.click(&item(1));
  agents.until("the tool runs the step", |s| {
    s.text("pipeline-hub-step-agent-name") == "Checks" && !s.exists(PICKER) && s.exists(CANCEL)
  });
  assert_eq!(
    agents.text("pipeline-hub-step-agent-runs"),
    "cargo · its exit code decides the step"
  );
  assert!(
    matches!(executor(&agents, ready, 0), StepKind::Agent { profile: Some(_), .. }),
    "nothing is stored before Save"
  );
  agents.click(CANCEL);
  agents.until("the profile is back", |s| {
    s.text("pipeline-hub-step-agent-name") == "Builder"
      && !s.text(&format!("pipeline-hub-row-{ready}-line")).starts_with("Unsaved")
  });
}

#[test]
fn choosing_a_profile_is_saved_with_the_pipeline() {
  let (mut agents, [ready, ..]) = setup();
  let other = profile_on(&agents, "Reviewer", Some(agents.listed().connections[0].id()));
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(1)));
  assert_eq!(agents.text(&format!("{}-name", item(1))), "Reviewer");
  agents.click(&item(1));
  agents.until("chosen", |s| s.text("pipeline-hub-step-agent-name") == "Reviewer");
  save_until(
    &mut agents,
    "saved",
    |s| matches!(executor(s, ready, 0), StepKind::Agent { profile: Some(id), .. } if id == other),
  );
  assert!(
    matches!(executor(&agents, ready, 0), StepKind::Agent { profile: Some(id), .. } if id == other),
    "the stored step runs the profile that was chosen"
  );
}

#[test]
fn a_profile_that_is_not_ready_says_why_in_the_list_and_can_still_be_chosen() {
  let (mut agents, [ready, ..]) = setup();
  profile_on(&agents, "Explorer", None);
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(1)));
  assert_eq!(agents.text(&format!("{}-name", item(1))), "Explorer");
  assert_eq!(
    agents.text(&format!("{}-runs", item(1))),
    "Choose a connection",
    "it says what to do, not what it runs on"
  );
  agents.click(&item(1));
  agents.until("chosen", |s| s.text("pipeline-hub-step-agent-name") == "Explorer");
  assert_eq!(
    agents.text("pipeline-hub-step-agent-runs"),
    "Choose a connection",
    "the field's own second line says it, in place of what it runs on"
  );
  assert!(
    !agents.exists("pipeline-hub-step-agent-row-sub"),
    "nothing is added under the label, so fixing the profile moves no row"
  );
  assert!(
    field_has(&mut agents, "not-ready"),
    "the field is outlined as not ready"
  );
}

/// Whether the Agent field carries the class `class`.
pub(super) fn field_has(agents: &mut Agents, class: &str) -> bool {
  agents
    .window
    .tree
    .get_element_by_id(FIELD)
    .is_some_and(|field| field.has_class(class))
}

/// A published profile `name` that chooses an MCP server `figma` whose program this computer does not have.
fn needing_a_program(agents: &Agents, name: &str) -> ResourceId {
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("figma").expect("name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("figma-mcp-not-installed").expect("program"),
      Vec::new(),
      None,
    )),
    McpEnvironment::new(BTreeMap::new()).expect("no variables"),
    Timestamp::now(),
  )
  .expect("server");
  let connection = agents.listed().connections[0].id();
  let profile = AgentProfile::new(
    Name::new(name).expect("name"),
    String::new(),
    PromptText::new("Do the work.").expect("instructions"),
    connection,
    ModelId::new("test-model").expect("model"),
    ProviderSettings::default(),
  )
  .with_mcp_servers(McpServerSelection::new(vec![server.id()]).expect("one server"));
  let mut db = ProjectDb::open(&agents.root).expect("database");
  db.create_mcp_server(&server).expect("the server");
  let id = db
    .create_resource(
      Slug::from_name(name).expect("slug"),
      Name::new(name).expect("name"),
      &profile,
      Timestamp::now(),
    )
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(id, Timestamp::now())
    .expect("published");
  id
}

#[test]
fn a_profile_that_lacks_something_here_warns_in_the_words_readiness_uses() {
  let (mut agents, [ready, ..]) = setup();
  needing_a_program(&agents, "Writer");
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(1)));
  assert_eq!(agents.text(&format!("{}-name", item(1))), "Writer");
  assert_eq!(
    agents.text(&format!("{}-runs", item(1))),
    "Set the program path of figma",
    "its connection is there, but what it chooses cannot start here"
  );
  assert!(
    agents.text(&format!("{}-runs", item(0))).ends_with("· test-model"),
    "Builder, which is ready, says what it runs on"
  );
  agents.click(&item(1));
  agents.until("chosen", |s| s.text("pipeline-hub-step-agent-name") == "Writer");
  assert_eq!(
    agents.text("pipeline-hub-step-agent-runs"),
    "Set the program path of figma"
  );
  assert!(field_has(&mut agents, "not-ready"));
}

#[test]
fn a_ready_profile_has_a_clean_field_with_no_outline_and_no_warning() {
  let (mut agents, _) = first_step();
  assert!(!field_has(&mut agents, "not-ready") && !field_has(&mut agents, "unset"));
  assert!(!agents.exists("pipeline-hub-step-agent-row-sub"));
}

#[test]
fn a_profile_that_was_archived_is_said_to_be_no_longer_available_not_to_be_nothing() {
  let (mut agents, _) = setup();
  let retired = profile_on(&agents, "Retired", Some(agents.listed().connections[0].id()));
  let pipeline = single(&agents, agent(retired, None));
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(retired, Timestamp::now())
    .expect("archived");
  let plan = keys(&agents, pipeline)[0].clone();
  open_step(&mut agents, pipeline, &plan);
  assert_eq!(
    agents.text("pipeline-hub-step-agent-name"),
    "Profile no longer available",
    "as the flow's card says it"
  );
  assert_eq!(
    agents.text("pipeline-hub-step-agent-runs"),
    "Choose another profile or tool",
    "and what to do"
  );
  assert!(
    field_has(&mut agents, "not-ready"),
    "the warning stroke, not the empty field's"
  );
  assert!(!field_has(&mut agents, "unset"));
  assert!(!agents.exists("pipeline-hub-step-agent-row-sub"));
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(1)));
  assert_eq!(
    agents.text(&format!("{}-name", item(1))),
    "Profile no longer available",
    "the picker lists what the step names, after the profiles the project has"
  );
}

#[test]
fn a_tool_that_was_archived_is_said_to_be_no_longer_available() {
  let (mut agents, _) = setup();
  let tool = checks(&agents);
  let pipeline = single(&agents, StepKind::Tool { tool: Some(tool) });
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(tool, Timestamp::now())
    .expect("archived");
  let plan = keys(&agents, pipeline)[0].clone();
  open_step(&mut agents, pipeline, &plan);
  assert_eq!(agents.text("pipeline-hub-step-agent-name"), "Tool no longer available");
  assert_eq!(
    agents.text("pipeline-hub-step-agent-runs"),
    "Choose another profile or tool"
  );
  assert!(field_has(&mut agents, "not-ready"));
}

#[test]
fn choosing_a_profile_for_a_step_that_overrides_the_model_keeps_the_override_and_says_it() {
  let (mut agents, _) = setup();
  let builder = agents.profile(true);
  let other = profile_on(&agents, "Reviewer", Some(agents.listed().connections[0].id()));
  let pipeline = overriding(&agents, builder, "gpt-override");
  let plan = keys(&agents, pipeline)[0].clone();
  open_step(&mut agents, pipeline, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(2)));
  let reviewer = (0..3)
    .find(|place| agents.text(&format!("{}-name", item(*place))) == "Reviewer")
    .expect("Reviewer is listed");
  assert!(
    !agents
      .text(&format!("{}-runs", item(reviewer)))
      .contains("gpt-override"),
    "only the step's own agent says it"
  );
  agents.click(&item(reviewer));
  agents.until("chosen", |s| s.text("pipeline-hub-step-agent-name") == "Reviewer");
  assert!(
    agents
      .text("pipeline-hub-step-agent-runs")
      .ends_with("· test-model · model: gpt-override"),
    "the override stays and shows on the new profile"
  );
  save_until(&mut agents, "saved", |s| {
    matches!(
      executor(s, pipeline, 0),
      StepKind::Agent { profile: Some(id), model: Some(model), .. } if id == other && model.as_str() == "gpt-override"
    )
  });
}

#[test]
fn a_tool_can_be_changed_back_to_a_profile() {
  let (mut agents, [ready, ..]) = setup();
  checks(&agents);
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(1)));
  agents.click(&item(1));
  agents.until("the tool", |s| s.text("pipeline-hub-step-agent-name") == "Checks");
  agents.click(FIELD);
  agents.until("the picker again", |s| s.exists(&item(0)));
  assert_eq!(agents.text(&format!("{}-name", item(0))), "Builder");
  agents.click(&item(0));
  agents.until("the profile is back", |s| {
    s.text("pipeline-hub-step-agent-name") == "Builder" && !s.exists(PICKER)
  });
  assert!(
    agents.text("pipeline-hub-step-agent-runs").ends_with("· test-model"),
    "it runs on what its profile says again"
  );
}

#[test]
fn a_press_outside_the_picker_closes_it_and_changes_nothing() {
  let (mut agents, [ready, ..]) = first_step();
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(PICKER));
  let (x, y, width, height) = rect(&mut agents, "pipeline-hub-step-position");
  let (x, y) = (x + width / 2.0, y + height / 2.0);
  agents.window.tree.mouse_move(x, y);
  agents.window.tree.mouse_down(x, y, MouseButton::Left);
  agents.window.tree.mouse_up(x, y, MouseButton::Left);
  agents.until("closed", |s| !s.exists(PICKER));
  assert!(
    !agents
      .text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved"),
    "nothing was changed"
  );
  assert!(matches!(
    executor(&agents, ready, 0),
    StepKind::Agent { profile: Some(_), .. }
  ));
}

#[test]
fn a_model_the_step_overrides_is_kept_and_said_after_the_profile_s_line() {
  let (mut agents, _) = setup();
  let profile = agents.profile(true);
  let pipeline = overriding(&agents, profile, "gpt-override");
  let plan = keys(&agents, pipeline)[0].clone();
  open_step(&mut agents, pipeline, &plan);
  let runs = agents.text("pipeline-hub-step-agent-runs");
  assert!(runs.ends_with("· test-model · model: gpt-override"), "{runs}");
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(0)));
  let chosen = (0..3).find(|place| {
    agents.exists(&item(*place))
      && agents
        .text(&format!("{}-runs", item(*place)))
        .ends_with("model: gpt-override")
  });
  assert!(chosen.is_some(), "the chosen row says the same");
  key(&mut agents, "Escape");
  assert!(
    matches!(
      stored(&agents, pipeline).steps()[0].kind.clone(),
      StepKind::Agent { model: Some(model), .. } if model.as_str() == "gpt-override"
    ),
    "nothing stored is changed by opening it"
  );
}

#[test]
fn the_foot_opens_the_new_profile_form_and_closes_the_picker() {
  let (mut agents, [ready, ..]) = setup();
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&format!("{PICKER}-new")));
  assert_eq!(agents.text(&format!("{PICKER}-agents")), "Agents ↗");
  agents.click(&format!("{PICKER}-new"));
  agents.until("the form", |s| {
    matches!(
      route(s),
      Route::Project(_, ProjectPage::AgentsHub(_) | ProjectPage::NewAgent(_))
    )
  });
  assert!(!agents.exists(PICKER), "the picker closed");
}

#[test]
fn the_foot_opens_the_agents_hub() {
  let (mut agents, [ready, ..]) = setup();
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&format!("{PICKER}-agents")));
  agents.click(&format!("{PICKER}-agents"));
  agents.until("the hub", |s| {
    matches!(
      route(s),
      Route::Project(_, ProjectPage::Agents | ProjectPage::AgentsHub(_))
    )
  });
  assert!(!agents.exists(PICKER), "the picker closed");
}

#[test]
fn leaving_by_the_foot_with_changes_that_are_not_saved_asks_first() {
  let (mut agents, [ready, ..]) = setup();
  checks(&agents);
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(1)));
  agents.click(&item(1));
  agents.until("changed", |s| s.exists(CANCEL));
  agents.click(FIELD);
  agents.until("the picker again", |s| s.exists(&format!("{PICKER}-agents")));
  let before = route(&agents);
  agents.click(&format!("{PICKER}-agents"));
  agents.until("the question", |s| s.exists("pipeline-hub-leave-discard"));
  assert_eq!(route(&agents), before, "nothing moved while it asks");
}
