//! The open step's ⋯ in its head (`Pipelines · v2` P2 `u2rYEH`, P2b `EGEos`, menu P4 `naN3G`; PV2-20): it opens the
//! flow card's menu for the step, whose rows act on the panel's edit; a copy or an added step opens, *Remove step*
//! leaves the panel on the pipeline's flow, and nothing moves when the ⋯ is pressed or the pointer is on it. With it,
//! two of PV2-18's polish: the question before a save brings the gate into view again when it is asked a second time,
//! and a very long name gives way so its pencil stays in view. `cargo test -p orchester-app --test agents
//! pipeline_step_more`.

use lurq::app::events::ScrollPhase;
use orchester_app::routes::PipelinesFilter;
use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{Pipeline, PipelineEdit, RepositoryAccess, Slug, StepKey, StepKind},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::pipelines::fields::RouteChoice;

use super::{
  harness::Agents,
  pipeline_hub::view,
  pipeline_step::{SAVE, chain, keys, named, open_step, route, save_until, step_route, stored},
  pipeline_step_gate::{claude, one_step},
};

const MORE: &str = "pipeline-hub-step-more";
const MENU: &str = "pipeline-hub-flow-menu";
const HINT: &str = "team-hub-actions-hint";

/// Where `id` is laid out: its start, top, width and height.
fn bounds(agents: &mut Agents, id: &str) -> (f32, f32, f32, f32) {
  let found = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (found.x, found.y, found.width, found.height)
}

/// A wheel up over the panel, as far as it goes.
fn to_top(agents: &mut Agents) {
  let (x, y, width, height) = bounds(agents, "pipeline-hub-panel-scroll");
  let scale = agents.window.tree.scale_factor();
  let (x, y) = ((x + width / 2.0) * scale, (y + height / 2.0) * scale);
  agents.window.tree.mouse_move(x, y);
  agents
    .window
    .tree
    .scroll(x, y, 0.0, 4000.0 * scale, ScrollPhase::Scroll);
  agents.window.pass();
}

/// Opens the head's menu.
fn open_menu(agents: &mut Agents) {
  agents.click(MORE);
  agents.until("the menu", |s| s.exists(MENU));
}

/// The middle step of three in series, open.
fn middle() -> (Agents, ResourceId, Vec<StepKey>) {
  let mut agents = Agents::new(true);
  let pipeline = chain(&agents, 3);
  let all = keys(&agents, pipeline);
  open_step(&mut agents, pipeline, &all[1]);
  (agents, pipeline, all)
}

#[test]
fn the_heads_more_opens_the_cards_menu_for_the_open_step_and_moves_nothing() {
  let (mut agents, ..) = middle();
  let before = [
    bounds(&mut agents, "pipeline-hub-step-duplicate"),
    bounds(&mut agents, MORE),
    bounds(&mut agents, "pipeline-hub-step-name-sizer"),
  ];
  let (x, y, width, height) = before[1];
  assert!(
    (width - 28.0).abs() < 0.5 && (height - 28.0).abs() < 0.5,
    "a 28 square: {width}×{height}"
  );
  agents.window.tree.mouse_move(x + width / 2.0, y + height / 2.0);
  agents.window.pass();
  open_menu(&mut agents);
  let after = [
    bounds(&mut agents, "pipeline-hub-step-duplicate"),
    bounds(&mut agents, MORE),
    bounds(&mut agents, "pipeline-hub-step-name-sizer"),
  ];
  assert_eq!(
    before, after,
    "the pointer on the ⋯ and its menu move nothing in the head"
  );

  // The card's rows: the moves with their keys, the neighbours, then the rest.
  assert!(agents.text("pipeline-hub-flow-menu-up").contains("Move up"));
  assert!(agents.text("pipeline-hub-flow-menu-up").contains("Alt ↑"));
  assert!(agents.text("pipeline-hub-flow-menu-down").contains("Move down"));
  assert!(
    agents.exists("pipeline-hub-flow-menu-parallel-0"),
    "a neighbour to run beside"
  );
  for (part, words) in [
    ("add", "Add a step after"),
    ("duplicate", "Duplicate"),
    ("remove", "Remove step"),
  ] {
    assert!(agents.text(&format!("{MENU}-{part}")).contains(words), "{part}");
  }
  agents.click(MORE);
  agents.until("closed", |s| !s.exists(MENU));
}

#[test]
fn a_move_from_the_head_changes_the_edit_and_the_step_stays_open() {
  let (mut agents, pipeline, all) = middle();
  open_menu(&mut agents);
  agents.click("pipeline-hub-flow-menu-up");
  agents.until("moved up", |s| s.text("pipeline-hub-step-position") == "step 01 of 3");
  assert_eq!(route(&agents), step_route(pipeline, &all[1]), "the step stays open");
  assert!(agents.text(HINT).starts_with("Unsaved"));
  save_until(&mut agents, "stored first", |s| {
    stored(s, pipeline)
      .step(&all[0])
      .is_some_and(|first| first.depends_on.contains(&all[1]))
  });
}

#[test]
fn a_copy_or_a_step_added_from_the_head_opens() {
  let (mut agents, ..) = middle();
  open_menu(&mut agents);
  agents.click("pipeline-hub-flow-menu-duplicate");
  agents.until("the copy is open", |s| named(s, "Review stage 2 copy"));
  assert_eq!(agents.text("pipeline-hub-step-position"), "step 03 of 4");

  open_menu(&mut agents);
  agents.click("pipeline-hub-flow-menu-add");
  agents.until("the new step is open", |s| named(s, "New step"));
  assert_eq!(agents.text("pipeline-hub-step-position"), "step 04 of 5");
}

#[test]
fn remove_step_from_the_head_leaves_the_panel_on_the_pipelines_flow() {
  let (mut agents, pipeline, all) = middle();
  open_menu(&mut agents);
  agents.click("pipeline-hub-flow-menu-remove");
  agents.until("the flow", |s| s.exists("pipeline-hub-flow"));
  assert_eq!(route(&agents), view(PipelinesFilter::All, Some(pipeline)));
  assert!(
    !agents.exists(&format!("pipeline-hub-flow-step-{}", all[1])),
    "its card is gone"
  );
  assert!(agents.text(HINT).starts_with("Unsaved"), "until Save");
}

#[test]
fn a_step_another_returns_to_cannot_be_removed_and_the_row_says_why() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let first = edit.add_step(Name::new("Build").expect("name"), None);
  let second = edit.add_step(Name::new("Review").expect("name"), Some(&first));
  for key in [&first, &second] {
    edit.update(key, |step| {
      step.kind = StepKind::Agent {
        profile: Some(builder),
        repository: RepositoryAccess::ReadOnly,
        model: None,
        permissions: StepPermissions::default(),
      };
    });
  }
  edit.update(&second, |step| RouteChoice(Some(first.clone())).apply(step));
  let mut db = ProjectDb::open(&agents.root).expect("database");
  // A pipeline is created as its first version (D-032).
  let id = db
    .create_published(
      Slug::from_name("Returns").expect("slug"),
      Name::new("Returns").expect("name"),
      &edit.validate().expect("a pipeline"),
      Timestamp::now(),
    )
    .expect("the pipeline")
    .0
    .id();
  open_step(&mut agents, id, &first);
  open_menu(&mut agents);
  assert!(
    agents.exists("pipeline-hub-flow-menu-remove-detail"),
    "the row says why it cannot be pressed"
  );
  agents.click("pipeline-hub-flow-menu-remove");
  agents.until("a pass", |_| true);
  assert_eq!(route(&agents), step_route(id, &first), "nothing was removed");
  assert!(!agents.text(HINT).starts_with("Unsaved"));
}

#[test]
fn while_the_question_waits_the_more_takes_no_press_and_a_second_question_brings_the_gate_into_view() {
  let mut agents = Agents::new(true);
  agents.window.tree.resize(1440, 700);
  let builder = claude(
    &agents,
    "Builder",
    RepositoryAccess::ReadOnly,
    &["cargo check", "cargo test"],
  );
  let asking = StepPermissions {
    interactive_tool_approvals: true,
    ..StepPermissions::default()
  };
  let (id, key) = one_step(&agents, builder, RepositoryAccess::ReadOnly, asking);
  open_step(&mut agents, id, &key);
  let in_view = |s: &mut Agents| {
    let (_, row_y, _, row_h) = bounds(s, "pipeline-hub-step-ask-tools-row");
    let (_, view_y, _, view_h) = bounds(s, "pipeline-hub-panel-scroll");
    row_y + row_h <= view_y + view_h + 0.5
  };
  assert!(!in_view(&mut agents), "the Ask row starts below the panel's view");
  agents.click("pipeline-hub-step-ask-tools");
  agents.press_until(SAVE, "the question", |s| s.exists("pipeline-hub-confirm-confirm"));
  agents.until("the gate in view", |s| in_view(s));
  agents.click(MORE);
  agents.until("a pass", |_| true);
  assert!(!agents.exists(MENU), "the ⋯ takes no press while the question waits");

  // Answered and asked again from the pipeline's view, then the step opened: the gate comes into view with it.
  agents.click("pipeline-hub-step-back");
  agents.until("the flow", |s| s.exists("pipeline-hub-flow"));
  agents.click("pipeline-hub-confirm-keep");
  agents.until("answered", |s| !s.exists("pipeline-hub-confirm-confirm"));
  agents.press_until(SAVE, "asked again", |s| s.exists("pipeline-hub-confirm-confirm"));
  // The panel keeps where it was scrolled to: its top, where a step opens.
  to_top(&mut agents);
  open_step(&mut agents, id, &key);
  agents.until("the gate in view again", |s| in_view(s));
}

#[test]
fn a_very_long_name_gives_way_and_its_pencil_stays_in_view() {
  let (mut agents, ..) = middle();
  agents.set("pipeline-hub-step-name", &"Long ".repeat(34));
  agents.until("renamed", |s| s.exists("pipeline-hub-step-name-edit"));
  let (panel_x, _, panel_w, _) = bounds(&mut agents, "pipeline-hub-panel-scroll");
  let edge = panel_x + panel_w;
  let (box_x, _, box_w, _) = bounds(&mut agents, "pipeline-hub-step-name-box");
  assert!(
    box_x + box_w <= edge,
    "the box ends in the panel: {} > {edge}",
    box_x + box_w
  );
  let (more_x, ..) = bounds(&mut agents, MORE);
  assert!(box_x + box_w <= more_x, "the name stops before the head's actions");
  let (pencil_x, _, pencil_w, _) = bounds(&mut agents, "pipeline-hub-step-name-edit");
  assert!(pencil_x + pencil_w <= more_x, "the pencil stays in view: {pencil_x}");
}

#[test]
fn alt_up_and_down_move_the_open_step_as_they_move_a_card() {
  let (mut agents, ..) = middle();
  super::pipeline_reorder::alt(&mut agents, "ArrowUp");
  agents.until("moved up", |s| s.text("pipeline-hub-step-position") == "step 01 of 3");
  super::pipeline_reorder::alt(&mut agents, "ArrowDown");
  agents.until("moved back down", |s| {
    s.text("pipeline-hub-step-position") == "step 02 of 3"
  });
}
