//! The open step's name, edited in place in its head (`Pipelines · v2` P2b `EGEos`, P2e `FHktg`, P2g `hYIuX`; PV2-18):
//! the pencil and the ring take turns and move nothing, Esc puts back the name the editing began with, Enter keeps it
//! and *Save* stores it; an empty or too long name is refused in place and holds *Save* until it is right.
//! `cargo test -p orchester-app --test agents pipeline_step_name`.

use orchester_core::resource::StepKey;

use super::{
  harness::Agents,
  pipeline_hub::setup,
  pipeline_step::{keys, named, open_step, save_until, stored},
};

const NAME: &str = "pipeline-hub-step-name";
const PENCIL: &str = "pipeline-hub-step-name-edit";
const HINT: &str = "pipeline-hub-step-name-hint";
const ERROR: &str = "pipeline-hub-step-name-error";
const SHOWN: &str = "pipeline-hub-step-name-shown";
const BAR: &str = "team-hub-actions-hint";

/// Where `id` is laid out: its start and top.
fn at(agents: &mut Agents, id: &str) -> (f32, f32) {
  let bounds = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (bounds.x, bounds.y)
}

fn focus(agents: &mut Agents) {
  agents
    .window
    .tree
    .get_element_by_id_mut(NAME)
    .expect("the name")
    .focus();
  agents.window.pass();
}

fn press(agents: &mut Agents, key: &str) {
  let tree = &mut agents.window.tree;
  tree.key_down(key.to_owned(), key.to_owned(), false, false, false);
  tree.key_up(key.to_owned(), key.to_owned(), false, false, false);
  agents.window.pass();
}

fn type_name(agents: &mut Agents, name: &str) {
  agents.set(NAME, name);
}

/// *Feature delivery*'s first step, open.
fn plan() -> (Agents, orchester_core::id::ResourceId, StepKey) {
  let (mut agents, [ready, ..]) = setup();
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  (agents, ready, plan)
}

#[test]
fn the_name_is_edited_in_place_and_nothing_moves_when_the_editing_begins() {
  let (mut agents, ready, plan) = plan();
  assert_eq!(agents.value(NAME), "Plan");
  assert!(agents.exists(PENCIL), "the pencil says the name can be renamed");
  assert!(!agents.exists(HINT));
  let (name, line, strip) = (
    at(&mut agents, "pipeline-hub-step-name-sizer"),
    at(&mut agents, "pipeline-hub-step-back"),
    at(&mut agents, "pipeline-hub-step-strip"),
  );
  assert!(
    (name.0 - line.0).abs() < 0.5,
    "the name starts where the line over it does: {name:?} {line:?}"
  );

  focus(&mut agents);
  agents.until("editing", |s| s.exists(HINT));
  assert_eq!(agents.text(HINT), "Enter to save · Esc to cancel");
  assert!(!agents.exists(PENCIL), "the pencil goes while the name is edited");
  assert_eq!(
    at(&mut agents, "pipeline-hub-step-name-sizer"),
    name,
    "the name does not move"
  );
  assert_eq!(
    at(&mut agents, "pipeline-hub-step-strip"),
    strip,
    "nothing under it moves"
  );

  type_name(&mut agents, "Planned");
  press(&mut agents, "Escape");
  agents.until("Esc puts the name back", |s| named(s, "Plan") && !s.exists(HINT));
  assert!(agents.exists(PENCIL));

  agents.click(PENCIL);
  agents.until("editing from the pencil", |s| s.exists(HINT));
  type_name(&mut agents, "Plan first");
  press(&mut agents, "Enter");
  agents.until("Enter ends the editing", |s| !s.exists(HINT));
  assert_eq!(agents.value(NAME), "Plan first", "and keeps the name");
  save_until(&mut agents, "the name stored", |s| {
    stored(s, ready).step(&plan).expect("the step").name.as_str() == "Plan first"
  });
}

#[test]
fn a_refused_name_says_why_holds_save_and_esc_puts_the_name_back() {
  let (mut agents, ready, plan) = plan();
  focus(&mut agents);
  agents.until("editing", |s| s.exists(HINT));
  type_name(&mut agents, "");
  agents.until("refused", |s| s.exists(ERROR));
  assert_eq!(agents.text(ERROR), "Enter a step name.");
  assert!(!agents.exists(HINT), "the error takes the hint's place");
  assert!(
    agents.text("team-hub-actions-hint").starts_with("Fix Name in Plan"),
    "Save waits: {}",
    agents.text("team-hub-actions-hint")
  );
  press(&mut agents, "Escape");
  agents.until("the name is back", |s| named(s, "Plan") && !s.exists(ERROR));

  focus(&mut agents);
  type_name(&mut agents, &"x".repeat(201));
  agents.until("too long", |s| s.exists(ERROR));
  assert_eq!(agents.text(ERROR), "Use at most 200 characters.");

  // Another step's name is a name like any other.
  type_name(&mut agents, "Build");
  agents.until("accepted", |s| !s.exists(ERROR));
  press(&mut agents, "Enter");
  save_until(&mut agents, "two steps called Build", |s| {
    stored(s, ready).step(&plan).expect("the step").name.as_str() == "Build"
  });
}

#[test]
fn a_refused_name_is_kept_while_another_step_is_open() {
  let (mut agents, ready, plan) = plan();
  focus(&mut agents);
  type_name(&mut agents, "");
  agents.until("refused", |s| s.exists(ERROR));
  press(&mut agents, "Enter");
  let build = keys(&agents, ready)[1].clone();
  agents.click(&format!("pipeline-hub-step-chip-{build}"));
  agents.until("the second step", |s| named(s, "Build"));
  assert!(!agents.exists(ERROR));
  assert!(agents.text("team-hub-actions-hint").starts_with("Fix Name in Plan"));
  agents.click(&format!("pipeline-hub-step-chip-{plan}"));
  agents.until("back, still refused", |s| named(s, "") && s.exists(ERROR));
}

#[test]
fn esc_with_nothing_focused_after_cancel_leaves_the_name_as_it_is() {
  let (mut agents, ..) = plan();
  agents.click("pipeline-hub-step-approval");
  agents.until("a change", |s| s.text(BAR).starts_with("Unsaved"));
  agents.click("pipeline-hub-cancel");
  agents.until("put back", |s| !s.text(BAR).starts_with("Unsaved"));
  press(&mut agents, "Escape");
  agents.until("a pass", |_| true);
  assert_eq!(
    agents.value(NAME),
    "Plan",
    "an Esc that ends no editing puts nothing back"
  );
  assert!(!agents.exists(ERROR));
  assert!(!agents.text(BAR).starts_with("Fix"));
}

#[test]
fn a_long_name_ends_in_an_ellipsis_with_the_whole_name_in_its_tooltip() {
  let (mut agents, ..) = plan();
  let long = "Long ".repeat(34).trim_end().to_owned();
  agents.set(NAME, &long);
  agents.until("the name as words", |s| s.exists(SHOWN));
  let (shown, room) = (
    at_box(&mut agents, SHOWN),
    at_box(&mut agents, "pipeline-hub-step-name-box"),
  );
  assert!(shown.2 <= room.2, "the words keep to the box: {shown:?} {room:?}");
  assert!(!agents.exists(&format!("{SHOWN}-tooltip")));
  let scale = agents.window.tree.scale_factor();
  let (x, y) = ((shown.0 + shown.2 / 2.0) * scale, (shown.1 + shown.3 / 2.0) * scale);
  agents.window.tree.mouse_move(x, y);
  agents.window.pass();
  agents
    .window
    .tree
    .tick_timers_at(std::time::Instant::now() + std::time::Duration::from_secs(3));
  agents.until("the whole name", |s| s.exists(&format!("{SHOWN}-tooltip")));
  assert_eq!(agents.text(&format!("{SHOWN}-tooltip")), long);
}

/// Where `id` is laid out: its start, top, width and height.
fn at_box(agents: &mut Agents, id: &str) -> (f32, f32, f32, f32) {
  let bounds = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (bounds.x, bounds.y, bounds.width, bounds.height)
}
