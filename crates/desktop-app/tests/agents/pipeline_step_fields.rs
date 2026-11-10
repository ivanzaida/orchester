//! The fields and chips of an open step that PV2-07's first tests leave: *Agent* and what it says when no profile is
//! chosen, *Retries* with the route they belong to, the step's *Sources*, *Duplicate* put back by *Cancel*, the Deliver
//! chip, and a chip pressed by the pointer in a strip that scrolls. `cargo test -p orchester-app --test agents
//! pipeline_step_fields`.

use lurq::app::events::MouseButton;
use orchester_app::routes::{PipelinesFilter, PipelinesView, ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  resource::{DeliverMode, DeliverStep, Draft, MergeMethod, Pipeline},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::{open, project, setup},
  pipeline_step::{CANCEL, SAVE, chain, keys, open_step, rect, route, save_until, step_route, stored},
  support::project_id,
};

#[test]
fn the_agent_menu_says_when_no_profile_is_chosen_and_choosing_one_changes_the_step() {
  // *Dependency update* is a draft whose steps run no profile yet.
  let (mut agents, [_, draft, _]) = setup();
  let plan = keys_of_draft(&agents, draft)[0].clone();
  agents.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(draft))),
  ));
  agents.window.navigation().go(step_route(draft, &plan));
  agents.until("the step", |s| s.exists("pipeline-hub-step-head"));
  assert_eq!(agents.text("pipeline-hub-step-agent-label"), "Choose a profile");
  assert_eq!(
    agents.text("pipeline-hub-step-agent-row-sub"),
    "No agent profile chosen yet"
  );

  agents.click("pipeline-hub-step-agent");
  agents.until("the menu", |s| s.exists("pipeline-hub-step-agent-option-1"));
  assert_eq!(agents.text("pipeline-hub-step-agent-option-1-label"), "Builder");
  agents.click("pipeline-hub-step-agent-option-1");
  agents.until("the profile is chosen", |s| {
    s.text("pipeline-hub-step-agent-label") == "Builder" && !s.exists("pipeline-hub-step-agent-row-sub")
  });
  agents.until("the edit holds it", |s| s.exists(CANCEL));
}

fn keys_of_draft(agents: &Agents, pipeline: ResourceId) -> Vec<orchester_core::resource::StepKey> {
  ProjectDb::open(&agents.root)
    .expect("database")
    .draft::<Pipeline>(pipeline)
    .expect("reads")
    .expect("a draft")
    .body()
    .steps()
    .iter()
    .map(|step| step.key.clone())
    .collect()
}

/// Whether the retries box is there and holds `text`.
fn retries_are(agents: &mut Agents, text: &str) -> bool {
  agents
    .window
    .tree
    .get_element_by_id_mut("pipeline-hub-step-retries")
    .and_then(|element| element.as_text_input())
    .is_some_and(|input| input.value() == text)
}

#[test]
fn retries_belong_to_the_route_and_go_with_it() {
  let mut agents = Agents::new(true);
  let ready = chain(&agents, 3);
  let all = keys(&agents, ready);
  let build = all[2].clone();
  open_step(&mut agents, ready, &build);
  assert!(!agents.exists("pipeline-hub-step-retries-row"), "no route, no retries");

  // The select is mounted anew after each save, which is in its id.
  let choose = |agents: &mut Agents, label: &str| {
    let id = (0..9)
      .map(|epoch| format!("pipeline-hub-step-changes-{build}-{epoch}"))
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
  };
  choose(&mut agents, "01 Review stage 1");
  agents.until("retries start at one", |s| retries_are(s, "1"));
  agents.set("pipeline-hub-step-retries", "3");
  save_until(&mut agents, "three retries stored", |s| {
    stored(s, ready)
      .step(&build)
      .expect("step")
      .on_failure
      .as_ref()
      .is_some_and(|route| route.max_retries.get() == 3)
  });

  // The panel lists what was stored before the next change: one made between the two would be put back with it.
  agents.until("the panel lists v2", |s| s.text("team-hub-actions-hint").contains("v3"));

  // Text that is not a number, then no route: the retries and what was wrong with them go, and Save is not held.
  agents.set("pipeline-hub-step-retries", "many");
  agents.until("the bar says what to fix", |s| {
    s.text("team-hub-actions-hint").starts_with("Fix Retries")
  });
  choose(&mut agents, "Fail the run");
  agents.until("the retries are gone", |s| !s.exists("pipeline-hub-step-retries-row"));
  agents.until("nothing to fix", |s| {
    !s.text("team-hub-actions-hint").starts_with("Fix")
  });
  save_until(&mut agents, "no route stored", |s| {
    stored(s, ready).step(&build).expect("step").on_failure.is_none()
  });

  agents.until("the panel lists v3", |s| s.text("team-hub-actions-hint").contains("v4"));

  // A route again starts at one retry, not at what was typed before.
  choose(&mut agents, "02 Review stage 2");
  agents.until("one retry", |s| retries_are(s, "1"));
}

#[test]
fn a_step_lists_its_own_sources_beside_the_pipelines_and_takes_them_off() {
  let (mut agents, [ready, ..]) = project();
  let plan = keys(&agents, ready)[0].clone();
  let (pipeline_folder, step_folder) = (
    tempfile::tempdir().expect("a folder"),
    tempfile::tempdir().expect("a folder"),
  );

  // The pipeline's own source, added where the pipeline's settings are.
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  agents.stand_ins.choose(Some(pipeline_folder.path().to_path_buf()));
  agents.press_until("pipeline-hub-sources-add-add", "its pill", |s| {
    s.exists("pipeline-hub-source-0")
  });

  agents.click(&format!("pipeline-hub-flow-step-{plan}"));
  agents.until("the step", |s| s.exists("pipeline-hub-step-sources-row"));
  assert_eq!(
    agents.text("pipeline-hub-step-sources-row-sub"),
    "0 own · 1 for every step"
  );
  assert!(
    !agents.exists("pipeline-hub-source-0"),
    "the pipeline's pill is not the step's"
  );

  agents.stand_ins.choose(Some(step_folder.path().to_path_buf()));
  agents.press_until("pipeline-hub-sources-add-add", "the step's pill", |s| {
    s.exists("pipeline-hub-source-0")
  });
  agents.until("counted", |s| {
    s.text("pipeline-hub-step-sources-row-sub") == "1 own · 1 for every step"
  });
  assert_eq!(
    agents.text("pipeline-hub-source-0"),
    step_folder.path().to_string_lossy(),
    "its own, by its path"
  );

  agents.click("pipeline-hub-source-0-remove");
  agents.until("taken off", |s| {
    !s.exists("pipeline-hub-source-0") && s.text("pipeline-hub-step-sources-row-sub") == "0 own · 1 for every step"
  });
}

#[test]
fn duplicate_names_the_copy_and_cancel_puts_the_flow_back_with_the_copy_open() {
  let (mut agents, [ready, ..]) = project();
  let [plan, _] = <[orchester_core::resource::StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);

  agents.click("pipeline-hub-step-duplicate");
  agents.until("the copy is open", |s| s.text("pipeline-hub-step-title") == "Plan copy");
  assert_eq!(
    agents.text("pipeline-hub-step-position").split(" of ").nth(1),
    Some("3")
  );

  // Cancel drops the copy: the step the route holds is gone, so the panel shows the pipeline, with its two steps.
  agents.click(CANCEL);
  agents.until("the pipeline", |s| {
    s.exists("pipeline-hub-flow") && !s.exists("pipeline-hub-step-head")
  });
  assert_eq!(
    route(&agents),
    Route::Project(
      project_id(),
      ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(ready)))
    )
  );
  assert_eq!(
    agents.text(&format!("pipeline-hub-row-{ready}-line")),
    "2 steps · used by 3 tasks"
  );
  assert_eq!(keys(&agents, ready).len(), 2, "nothing was stored");
}

/// The ready pipeline's latest version, which ends in a Deliver step the draft adds.
fn delivering(agents: &mut Agents) -> ResourceId {
  let pipeline = chain(agents, 3);
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let latest = db
    .latest_version::<Pipeline>(pipeline)
    .expect("reads")
    .expect("a version");
  let body = latest.body().clone().with_deliver(Some(DeliverStep::new(
    DeliverMode::AutoMerge,
    MergeMethod::Squash,
    None,
    true,
  )));
  db.save_draft(pipeline, &Draft::new(body, Some(latest.number()), Timestamp::now()))
    .expect("a draft");
  pipeline
}

#[test]
fn the_deliver_chip_opens_the_pipeline_keeps_the_edit_and_shows_what_a_run_delivers() {
  let mut agents = Agents::new(true);
  let pipeline = delivering(&mut agents);
  let first = keys(&agents, pipeline)[0].clone();
  open_step(&mut agents, pipeline, &first);
  agents.set(super::pipeline_step::INSTRUCTIONS, "Typed before Deliver");
  agents.until("held", |s| {
    s.text(&format!("pipeline-hub-row-{pipeline}-line"))
      .starts_with("Unsaved changes")
  });

  agents.click("pipeline-hub-step-chip-deliver");
  agents.until("the pipeline", |s| s.exists("pipeline-hub-flow"));
  assert_eq!(
    route(&agents),
    Route::Project(
      project_id(),
      ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(pipeline)))
    )
  );
  assert!(
    !agents.exists("pipeline-hub-leave"),
    "no question: the edit stays where it is"
  );
  assert!(
    agents
      .text(&format!("pipeline-hub-row-{pipeline}-line"))
      .starts_with("Unsaved changes")
  );

  // The row of what a run delivers is brought into view, inside the panel's scroll.
  agents.until("A run delivers is in view", |s| {
    let (row, scroll) = (
      rect_y(s, "pipeline-hub-delivers"),
      rect_y(s, "pipeline-hub-panel-scroll"),
    );
    row.0 >= scroll.0 - 0.5 && row.0 + row.1 <= scroll.0 + scroll.1 + 0.5
  });
}

fn rect_y(agents: &mut Agents, id: &str) -> (f32, f32) {
  let found = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (found.y, found.height)
}

#[test]
fn a_chip_pressed_by_the_pointer_opens_its_step_in_a_strip_that_scrolls() {
  let mut agents = Agents::new(true);
  let long = chain(&agents, 16);
  let all = keys(&agents, long);
  open_step(&mut agents, long, &all[15]);
  agents.until("the last step is in view", |s| {
    s.exists("pipeline-hub-step-strip-before")
  });
  agents.until("laid out", |s| {
    rect(s, "pipeline-hub-step-strip-before-panel").width > 0.0
  });

  // The panels are their own 36 squares at the strip's ends, not the strip's whole width.
  let (strip, left) = (
    rect(&mut agents, "pipeline-hub-step-scroll"),
    rect(&mut agents, "pipeline-hub-step-strip-before-panel"),
  );
  assert!(left.width < 40.0, "‹ takes only its own panel: {left:?} of {strip:?}");

  // A chip between the panels is pressed with the pointer, as a person does, and opens its step.
  let target = all[13].clone();
  let chip = format!("pipeline-hub-step-chip-{target}");
  let (x, y) = {
    let found = agents
      .window
      .tree
      .get_element_by_id_mut(&chip)
      .and_then(|element| element.bounds())
      .expect("the chip");
    found.center()
  };
  agents.window.tree.mouse_move(x, y);
  agents.window.tree.mouse_down(x, y, MouseButton::Left);
  agents.window.tree.mouse_up(x, y, MouseButton::Left);
  agents.until("its step", |s| route(s) == step_route(long, &target));
}

#[test]
fn one_arrow_press_moves_one_screen_not_to_the_end() {
  let mut agents = Agents::new(true);
  let long = chain(&agents, 16);
  let all = keys(&agents, long);
  open_step(&mut agents, long, &all[15]);
  agents.until("scrolled to the end", |s| {
    s.exists("pipeline-hub-step-strip-before") && !s.exists("pipeline-hub-step-strip-after")
  });
  let chip = format!("pipeline-hub-step-chip-{}", all[15]);
  let before = rect(&mut agents, &chip).x;
  let room = rect(&mut agents, "pipeline-hub-step-scroll").width;

  agents.click("pipeline-hub-step-strip-before");
  agents.until("moved", |s| rect(s, &chip).x > before + 50.0);
  let moved = rect(&mut agents, &chip).x - before;
  assert!(
    (moved - (room - 80.0)).abs() < 2.0,
    "one screen is the room between the panels ({}): moved {moved}",
    room - 80.0
  );
  assert!(
    agents.exists("pipeline-hub-step-strip-after"),
    "and there is more after it now"
  );
}
