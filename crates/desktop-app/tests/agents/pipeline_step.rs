//! A step opened in the Pipelines hub's panel (`Pipelines · v2` P2 `u2rYEH`, P13 `KRQ5J`; PV2-07): the route holds the
//! open step, the head returns to the pipeline, the strip switches steps and scrolls when the flow is longer than it,
//! and the fields change the edit the panel holds until *Save*. `cargo test -p orchester-app --test agents
//! pipeline_step`.

use orchester_app::routes::{PipelineView, PipelinesFilter, PipelinesView, ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{Pipeline, PipelineEdit, RepositoryAccess, Slug, StepKey, StepKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::{open, setup},
  support::project_id,
};

const SAVE: &str = "pipeline-hub-save";
const CANCEL: &str = "pipeline-hub-cancel";
const INSTRUCTIONS: &str = "pipeline-hub-step-instructions";

fn step_route(pipeline: ResourceId, key: &StepKey) -> Route {
  Route::Project(
    project_id(),
    ProjectPage::Pipeline(pipeline, PipelineView::Step(key.clone())),
  )
}

/// The steps of `pipeline` as stored now: its latest version.
fn stored(agents: &Agents, pipeline: ResourceId) -> Pipeline {
  ProjectDb::open(&agents.root)
    .expect("database")
    .latest_version::<Pipeline>(pipeline)
    .expect("reads")
    .expect("a version")
    .body()
    .clone()
}

fn keys(agents: &Agents, pipeline: ResourceId) -> Vec<StepKey> {
  stored(agents, pipeline)
    .steps()
    .iter()
    .map(|step| step.key.clone())
    .collect()
}

/// Opens step `key` of `pipeline` from the hub, as a card's press does, and waits for the step's head.
fn open_step(agents: &mut Agents, pipeline: ResourceId, key: &StepKey) {
  open(agents, PipelinesFilter::All, Some(pipeline));
  agents.window.navigation().go(step_route(pipeline, key));
  agents.until("the step", |s| {
    s.exists("pipeline-hub-step-head") && s.exists("pipeline-hub-step-strip")
  });
}

/// Presses *Save* and, when it asks a person to allow what the save changes (instructions, a budget, the steps),
/// *Allow*, until `done`.
fn save_until(agents: &mut Agents, what: &str, done: impl Fn(&mut Agents) -> bool) {
  agents.until(what, |s| {
    if done(s) {
      return true;
    }
    for id in ["pipeline-hub-confirm-confirm", SAVE] {
      if let Some(mut button) = s.window.tree.get_element_by_id_mut(id) {
        button.click();
        break;
      }
    }
    s.window.pass();
    done(s)
  });
}

fn route(agents: &Agents) -> Route {
  agents.window.navigation().route_now()
}

/// A published pipeline of `count` steps run one after the other, named `Step 01 …`, which the profile builds.
fn chain(agents: &Agents, count: usize) -> ResourceId {
  let builder = agents.profile(true);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let mut before: Option<StepKey> = None;
  for number in 1..=count {
    let name = format!("Review stage {number}");
    let key = edit.add_step(Name::new(&name).expect("name"), before.as_ref());
    edit.update(&key, |step| {
      step.kind = StepKind::Agent {
        profile: Some(builder),
        repository: RepositoryAccess::ReadOnly,
        model: None,
        permissions: StepPermissions::default(),
      };
    });
    before = Some(key);
  }
  let body = edit.validate().expect("a pipeline");
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = db
    .create_resource(
      Slug::from_name("Long release").expect("slug"),
      Name::new("Long release").expect("name"),
      &body,
      Timestamp::now(),
    )
    .expect("the pipeline")
    .id();
  db.publish_draft::<Pipeline>(id, Timestamp::now()).expect("published");
  id
}

#[derive(Clone, Copy, Debug)]
struct Rect {
  x: f32,
  width: f32,
}

fn rect(agents: &mut Agents, id: &str) -> Rect {
  let found = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  Rect {
    x: found.x,
    width: found.width,
  }
}

/// Whether chip `id` shows whole in the strip's scrolling part.
fn in_view(agents: &mut Agents, id: &str) -> bool {
  let (chip, strip) = (rect(agents, id), rect(agents, "pipeline-hub-step-scroll"));
  chip.x >= strip.x - 0.5 && chip.x + chip.width <= strip.x + strip.width + 0.5
}

#[test]
fn a_step_opens_in_the_panel_with_its_head_strip_and_groups() {
  let (mut agents, [ready, ..]) = setup();
  let [plan, build] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);

  assert_eq!(agents.text("pipeline-hub-step-back-name"), "Feature delivery");
  assert_eq!(agents.text("pipeline-hub-step-position"), "step 01 of 2");
  assert_eq!(agents.text("pipeline-hub-step-title"), "Plan");
  assert!(agents.exists("pipeline-hub-step-duplicate"));
  assert!(!agents.exists("pipeline-hub-flow"), "the flow gives way to the step");
  assert!(
    !agents.exists("pipeline-hub-step-after-row"),
    "a step has no Runs after"
  );

  assert_eq!(agents.text(&format!("pipeline-hub-step-chip-{plan}")), "01 Plan");
  assert_eq!(agents.text(&format!("pipeline-hub-step-chip-{build}")), "02 Build");

  for (id, words) in [
    ("pipeline-hub-step-group-label", "Step"),
    ("pipeline-hub-step-agent-row-label", "Agent"),
    ("pipeline-hub-step-instructions-row-label", "Instructions"),
    (
      "pipeline-hub-step-instructions-row-sub",
      "{{ }} inserts task and run values",
    ),
    ("pipeline-hub-step-run-label", "Run"),
    ("pipeline-hub-step-changes-row-label", "When changes are requested"),
    ("pipeline-hub-step-budget-row-label", "Step budget"),
    ("pipeline-hub-step-gate-label", "Gate and access"),
    ("pipeline-hub-step-approval-row-label", "Wait for your approval"),
    ("pipeline-hub-step-access-row-label", "Repository access"),
    ("pipeline-hub-step-sources-row-label", "Sources"),
    ("pipeline-hub-step-sources-row-sub", "0 own · 0 for every step"),
  ] {
    assert_eq!(agents.text(id), words, "{id}");
  }
  assert_eq!(agents.text("pipeline-hub-step-agent-label"), "Builder");
  assert!(
    agents.exists(SAVE),
    "Save and Cancel stand under the step as under the pipeline"
  );
}

#[test]
fn the_route_holds_the_open_step_and_back_returns_to_the_pipeline() {
  let (mut agents, [ready, ..]) = setup();
  let [plan, _] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);
  assert_eq!(route(&agents), step_route(ready, &plan));

  agents.click("pipeline-hub-step-back");
  agents.until("the pipeline", |s| s.exists("pipeline-hub-flow"));
  assert_eq!(
    route(&agents),
    Route::Project(
      project_id(),
      ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(ready)))
    )
  );
  assert!(!agents.exists("pipeline-hub-step-head"));

  // A card of the flow opens the step again in the panel: no page of its own.
  agents.click(&format!("pipeline-hub-flow-step-{plan}"));
  agents.until("the step", |s| s.exists("pipeline-hub-step-head"));
  assert_eq!(route(&agents), step_route(ready, &plan));
}

#[test]
fn a_chip_switches_steps_and_the_keys_walk_through_them() {
  let (mut agents, [ready, ..]) = setup();
  let [plan, build] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);

  agents.click(&format!("pipeline-hub-step-chip-{build}"));
  agents.until("the second step", |s| s.text("pipeline-hub-step-title") == "Build");
  assert_eq!(route(&agents), step_route(ready, &build));
  assert_eq!(agents.text("pipeline-hub-step-position"), "step 02 of 2");

  super::pipeline_hub_menu::key(&mut agents, "ArrowLeft");
  agents.until("the first step", |s| s.text("pipeline-hub-step-title") == "Plan");
  super::pipeline_hub_menu::key(&mut agents, "ArrowLeft");
  assert_eq!(
    agents.text("pipeline-hub-step-title"),
    "Plan",
    "nothing runs before the first step"
  );
  super::pipeline_hub_menu::key(&mut agents, "ArrowRight");
  agents.until("the second step again", |s| {
    s.text("pipeline-hub-step-title") == "Build"
  });
  super::pipeline_hub_menu::key(&mut agents, "ArrowRight");
  assert_eq!(
    agents.text("pipeline-hub-step-title"),
    "Build",
    "nothing runs after the last step"
  );
}

#[test]
fn what_is_typed_in_a_step_is_held_across_steps_and_the_pipeline_until_save() {
  let (mut agents, [ready, ..]) = setup();
  let [plan, build] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);

  agents.set(INSTRUCTIONS, "Write the plan for {{task.title}}");
  agents.until("the row says it is not saved", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });

  agents.click(&format!("pipeline-hub-step-chip-{build}"));
  agents.until("the second step", |s| s.text("pipeline-hub-step-title") == "Build");
  assert_eq!(agents.value(INSTRUCTIONS), "", "another step has its own text");
  agents.click("pipeline-hub-step-back");
  agents.until("the pipeline", |s| s.exists("pipeline-hub-flow"));
  agents.until("going back asks nothing and keeps the edit", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });
  assert!(!agents.exists("pipeline-hub-leave"), "no question about leaving");
  agents.click(&format!("pipeline-hub-flow-step-{plan}"));
  agents.until("the first step again", |s| s.exists(INSTRUCTIONS));
  assert_eq!(agents.value(INSTRUCTIONS), "Write the plan for {{task.title}}");

  assert_eq!(
    stored(&agents, ready).step(&plan).expect("step").instructions.as_str(),
    "",
    "nothing is stored before Save"
  );
  save_until(&mut agents, "the next version", |s| {
    stored(s, ready).step(&plan).expect("step").instructions.as_str() == "Write the plan for {{task.title}}"
  });
}

#[test]
fn cancel_puts_back_what_the_step_held() {
  let (mut agents, [ready, ..]) = setup();
  let [plan, _] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);

  agents.set(INSTRUCTIONS, "Never saved");
  agents.until("Cancel is offered", |s| s.exists(CANCEL));
  agents.click(CANCEL);
  agents.until("what is stored shows again", |s| {
    s.value(INSTRUCTIONS).is_empty() && !s.text(&format!("pipeline-hub-row-{ready}-line")).starts_with("Unsaved")
  });
}

#[test]
fn a_placeholder_that_names_no_variable_is_said_under_the_box_and_keeps_save_from_storing_it() {
  let (mut agents, [ready, ..]) = setup();
  let [plan, _] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);

  agents.set(INSTRUCTIONS, "Start with the valid {{task.title}}");
  agents.until("Save is offered", |s| s.exists(SAVE));
  agents.set(INSTRUCTIONS, "Then {{nonsense.thing}}");
  agents.until("the check is under the box", |s| {
    s.window
      .texts("pipeline-hub-step-instructions-note")
      .is_some_and(|words| words.join(" ").contains("nonsense.thing"))
  });
  for _ in 0..3 {
    agents.click(SAVE);
  }
  assert!(
    stored(&agents, ready)
      .step(&plan)
      .expect("step")
      .instructions
      .as_str()
      .is_empty(),
    "nothing was stored while the text is wrong"
  );

  agents.set(INSTRUCTIONS, "Now {{task.title}} is right");
  agents.until("the check is gone", |s| {
    !s.exists("pipeline-hub-step-instructions-note")
  });
  save_until(&mut agents, "the text stored", |s| {
    stored(s, ready).step(&plan).expect("step").instructions.as_str() == "Now {{task.title}} is right"
  });
}

#[test]
fn the_budget_the_approval_the_access_and_the_route_are_held_by_the_edit() {
  let mut agents = Agents::new(true);
  let ready = chain(&agents, 3);
  let [plan, build, _] = <[StepKey; 3]>::try_from(keys(&agents, ready)).expect("three steps");
  open_step(&mut agents, ready, &build);

  agents.set("pipeline-hub-step-budget", "12000");
  agents.click("pipeline-hub-step-approval");
  agents.until("the approval is on", |s| s.exists(SAVE));

  // *When changes are requested* offers the steps before this one; choosing one gives the step its retries.
  let mut select = agents
    .window
    .tree
    .get_element_by_id_mut(&format!("pipeline-hub-step-changes-{build}-0"))
    .and_then(|element| element.as_select())
    .expect("the select");
  let labels: Vec<String> = select.labels().iter().map(|label| label.to_string()).collect();
  assert_eq!(labels, ["Fail the run", "01 Review stage 1"]);
  select.commit(1);
  agents.window.pass();
  agents.until("the retries row", |s| s.exists("pipeline-hub-step-retries-row"));

  save_until(&mut agents, "the next version", |s| {
    stored(s, ready).step(&build).expect("step").approval_gate
  });
  let saved = stored(&agents, ready);
  let step = saved.step(&build).expect("step");
  assert_eq!(step.budget.map(|budget| budget.get()), Some(12_000));
  assert_eq!(
    step.on_failure.as_ref().map(|route| route.return_to.clone()),
    Some(plan)
  );
}

#[test]
fn duplicate_copies_the_step_after_it_and_opens_the_copy() {
  let (mut agents, [ready, ..]) = setup();
  let [plan, _] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);

  agents.click("pipeline-hub-step-duplicate");
  agents.until("the copy is open", |s| {
    route(s) != step_route(ready, &plan) && s.exists("pipeline-hub-step-head")
  });
  assert_eq!(
    agents.text("pipeline-hub-step-position").split(" of ").nth(1),
    Some("3")
  );
  assert_eq!(keys(&agents, ready).len(), 2, "nothing is stored before Save");
  save_until(&mut agents, "the copy stored", |s| stored(s, ready).steps().len() == 3);
}

#[test]
fn a_route_naming_a_step_the_pipeline_has_not_shows_the_pipeline() {
  let (mut agents, [ready, ..]) = setup();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  let missing = StepKey::new("no-such-step").expect("key");
  agents.window.navigation().go(step_route(ready, &missing));
  agents.until("the pipeline", |s| s.exists("pipeline-hub-flow"));
  assert_eq!(
    route(&agents),
    Route::Project(
      project_id(),
      ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(ready)))
    )
  );
}

#[test]
fn a_long_pipeline_scrolls_its_strip_and_opens_with_the_open_step_in_view() {
  let mut agents = Agents::new(true);
  let long = chain(&agents, 16);
  let all = keys(&agents, long);
  let last = all.last().expect("steps").clone();
  open_step(&mut agents, long, &last);
  agents.until("the last step is in view", |s| {
    in_view(s, &format!("pipeline-hub-step-chip-{last}"))
  });
  let first = format!("pipeline-hub-step-chip-{}", all[0]);
  assert!(!in_view(&mut agents, &first), "the strip is longer than its room");
  assert!(agents.exists("pipeline-hub-step-strip-before"), "there is more before");
  assert!(
    !agents.exists("pipeline-hub-step-strip-after"),
    "nothing more after the last step"
  );

  // ‹ moves by one screen and › shows once there is more after.
  let before = rect(&mut agents, &format!("pipeline-hub-step-chip-{last}")).x;
  agents.click("pipeline-hub-step-strip-before");
  agents.until("the strip moved", |s| {
    rect(s, &format!("pipeline-hub-step-chip-{last}")).x > before + 100.0
  });
  assert!(agents.exists("pipeline-hub-step-strip-after"));

  // Another step switches the open one and the strip keeps it in view.
  let middle = all[8].clone();
  agents.window.navigation().go(step_route(long, &middle));
  agents.until("the middle step is in view", |s| {
    s.text("pipeline-hub-step-title") == "Review stage 9" && in_view(s, &format!("pipeline-hub-step-chip-{middle}"))
  });
}

#[test]
fn a_short_pipeline_has_no_arrows() {
  let (mut agents, [ready, ..]) = setup();
  let [plan, _] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);
  assert!(!agents.exists("pipeline-hub-step-strip-before"));
  assert!(!agents.exists("pipeline-hub-step-strip-after"));
}
