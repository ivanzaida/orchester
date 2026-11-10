//! The step's *Agent* field and the Agent picker under it (`Pipelines · v2` P2 `u2rYEH`, P10 `HjgZ8`, K4 `CwLfL`;
//! PV2-08): the profile's name and what it runs on, the picker opening under the field from its start, choosing a
//! profile or a tool (also one that is not ready) changing the edit that *Cancel* puts back, a model the step overrides
//! kept and said, and the foot's two ways out. `cargo test -p orchester-app --test agents pipeline_step_agent`.

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, CommandAction, CommandProgram, ModelId, Pipeline, PipelineEdit, PromptText, ProviderSettings,
    RepositoryAccess, Slug, StepKind, Tool, ToolAction,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::setup,
  pipeline_hub_menu::key,
  pipeline_step::{CANCEL, keys, open_step, route, save_until, stored},
};

const FIELD: &str = "pipeline-hub-step-agent";
const PICKER: &str = "pipeline-hub-step-agent-picker";

fn item(place: usize) -> String {
  format!("{PICKER}-item-{place}")
}

/// What the step at `place` of the stored pipeline runs.
fn executor(agents: &Agents, pipeline: ResourceId, place: usize) -> StepKind {
  stored(agents, pipeline).steps()[place].kind.clone()
}

/// A command tool `Checks` that runs `cargo`, published.
fn checks(agents: &Agents) -> ResourceId {
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
fn profile_on(agents: &Agents, name: &str, connection: Option<ConnectionId>) -> ResourceId {
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
fn overriding(agents: &Agents, profile: ResourceId, model: &str) -> ResourceId {
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let key = edit.add_step(Name::new("Plan").expect("name"), None);
  edit.update(&key, |step| {
    step.kind = StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::ReadOnly,
      model: Some(ModelId::new(model).expect("model")),
      permissions: StepPermissions::default(),
    };
  });
  let body = edit.validate().expect("a pipeline");
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = db
    .create_resource(
      Slug::new("override").expect("slug"),
      Name::new("Override").expect("name"),
      &body,
      Timestamp::now(),
    )
    .expect("the pipeline")
    .id();
  db.publish_draft::<Pipeline>(id, Timestamp::now()).expect("published");
  id
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
    agents.text("pipeline-hub-step-agent-row-sub"),
    "Choose a connection",
    "the row says it too"
  );
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
