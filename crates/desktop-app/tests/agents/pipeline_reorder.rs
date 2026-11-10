//! Changing the order of a pipeline's steps in the flow (`Pipelines · v2` P3 `K4B56`, P4 `naN3G`; PV2-06): a hovered
//! card shows its handle and ⋯; the ⋯ menu lists the moves a step has where it stands; Alt ↑ / Alt ↓ move the card the
//! keys are on; dragging the handle lands a card between two places or on a card; a refused move says why. Every change
//! goes through the edit the panel holds, so nothing is stored until Save. `cargo test -p orchester-app --test agents
//! pipeline_reorder`.

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, RepositoryAccess, StepKey},
  store::ResourceStore,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_flow::{add, agent, fails_back_to, open, store},
};

/// research → plan → build → { checks ∥ review }: numbered 01 … 05.
pub(super) fn flow(agents: &Agents, plan_returns: bool) -> (ResourceId, [StepKey; 5]) {
  let profile = Some(agents.profile(true));
  let kind = || agent(profile, RepositoryAccess::ReadOnly);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let research = add(&mut edit, "Research", &[], kind());
  let plan = add(&mut edit, "Plan", &[&research], kind());
  let build = add(&mut edit, "Build", &[&plan], kind());
  let checks = add(&mut edit, "Run checks", &[&build], kind());
  let review = add(&mut edit, "Review", &[&build], kind());
  if plan_returns {
    fails_back_to(&mut edit, &plan, &research);
  }
  let body = edit.validate().expect("a pipeline");
  (
    store(agents, "Feature", &body, true),
    [research, plan, build, checks, review],
  )
}

pub(super) fn card(key: &StepKey) -> String {
  format!("pipeline-hub-flow-step-{key}")
}

/// The cards among `keys` from the top of the flow down; a group's cards in the order they stand in.
pub(super) fn order(agents: &mut Agents, keys: &[StepKey]) -> Vec<StepKey> {
  let mut placed: Vec<(f32, f32, StepKey)> = keys
    .iter()
    .map(|key| {
      let found = agents
        .window
        .tree
        .get_element_by_id_mut(&card(key))
        .and_then(|found| found.bounds())
        .expect("laid out");
      (found.y, found.x, key.clone())
    })
    .collect();
  placed.sort_by(|a, b| a.partial_cmp(b).expect("positions compare"));
  placed.into_iter().map(|(.., key)| key).collect()
}

pub(super) fn point(agents: &mut Agents, x: f32, y: f32) -> (f32, f32) {
  let scale = agents.window.tree.scale_factor();
  (x * scale, y * scale)
}

/// Rests the pointer on the middle of `id`.
pub(super) fn hover(agents: &mut Agents, id: &str) {
  let bounds = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|found| found.bounds())
    .unwrap_or_else(|| panic!("{id} laid out"));
  let (x, y) = point(agents, bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  agents.window.tree.mouse_move(x, y);
  agents.window.pass();
}

/// Rests the pointer on `key`'s card and presses its ⋯.
pub(super) fn open_menu(agents: &mut Agents, key: &StepKey) {
  hover(agents, &card(key));
  let more = format!("{}-more", card(key));
  agents.until("the ⋯", |s| s.exists(&more));
  agents.click(&more);
  agents.until("the menu", |s| s.exists("pipeline-hub-flow-menu"));
}

pub(super) fn close_menu(agents: &mut Agents) {
  agents
    .window
    .tree
    .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  agents.until("the menu closed", |s| !s.exists("pipeline-hub-flow-menu"));
}

pub(super) fn alt(agents: &mut Agents, key: &str) {
  agents
    .window
    .tree
    .key_down(key.to_owned(), key.to_owned(), false, false, true);
  agents.window.pass();
}

pub(super) fn focus(agents: &mut Agents, key: &StepKey) {
  agents
    .window
    .tree
    .get_element_by_id_mut(&card(key))
    .expect("the card")
    .focus();
  agents.window.pass();
}

pub(super) fn stored(agents: &Agents, id: ResourceId) -> Pipeline {
  let db = ProjectDb::open(&agents.root).expect("database");
  match db.draft::<Pipeline>(id).expect("draft") {
    Some(draft) => draft.body().clone(),
    None => db
      .latest_version::<Pipeline>(id)
      .expect("version")
      .expect("a version")
      .body()
      .clone(),
  }
}

pub(super) fn depends(pipeline: &Pipeline, key: &StepKey) -> Vec<String> {
  pipeline
    .steps()
    .iter()
    .find(|step| &step.key == key)
    .expect("the step")
    .depends_on
    .iter()
    .map(ToString::to_string)
    .collect()
}

#[test]
fn pipeline_reorder_a_card_shows_its_handle_and_menu_only_where_the_pointer_or_keys_are() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, ..]) = flow(&agents, false);
  open(&mut agents, id);
  assert!(
    !agents.exists(&format!("{}-handle", card(&research))),
    "no handle at rest"
  );
  assert!(!agents.exists(&format!("{}-more", card(&research))), "no ⋯ at rest");

  hover(&mut agents, &card(&research));
  agents.until("the handle", |s| s.exists(&format!("{}-handle", card(&research))));
  assert!(agents.exists(&format!("{}-more", card(&research))));
  assert!(
    !agents.exists(&format!("{}-handle", card(&plan))),
    "only the card the pointer is on"
  );

  focus(&mut agents, &plan);
  agents.until("the keys' card", |s| s.exists(&format!("{}-more", card(&plan))));
}

#[test]
fn pipeline_reorder_the_menu_lists_the_moves_a_step_has_where_it_stands() {
  let mut agents = Agents::new(true);
  let (id, [research, _, build, _, review]) = flow(&agents, false);
  open(&mut agents, id);

  // The first step: nothing before it, one neighbour to join.
  open_menu(&mut agents, &research);
  assert_eq!(
    agents.text("pipeline-hub-flow-menu-up-detail"),
    "Nothing runs before it",
    "Move up says why it does nothing"
  );
  assert!(
    !agents.exists("pipeline-hub-flow-menu-down-detail"),
    "Move down is offered"
  );
  assert_eq!(
    agents.text("pipeline-hub-flow-menu-parallel-0-label"),
    "Run in parallel with 02 Plan"
  );
  assert!(
    !agents.exists("pipeline-hub-flow-menu-parallel-1"),
    "only the neighbours it can join"
  );
  assert!(
    !agents.exists("pipeline-hub-flow-menu-own"),
    "it is not in a parallel group"
  );
  for item in ["add", "duplicate", "remove"] {
    assert!(agents.exists(&format!("pipeline-hub-flow-menu-{item}")), "{item}");
  }
  close_menu(&mut agents);

  // A step in the middle joins the one before it or the group after it.
  open_menu(&mut agents, &build);
  assert!(!agents.exists("pipeline-hub-flow-menu-up-detail"));
  assert!(!agents.exists("pipeline-hub-flow-menu-down-detail"));
  assert_eq!(
    agents.text("pipeline-hub-flow-menu-parallel-0-label"),
    "Run in parallel with 02 Plan"
  );
  assert_eq!(
    agents.text("pipeline-hub-flow-menu-parallel-1-label"),
    "Run in parallel with 04 and 05"
  );
  assert!(!agents.exists("pipeline-hub-flow-menu-own"));
  close_menu(&mut agents);

  // A step in a parallel group can run on its own, and has nothing after it.
  open_menu(&mut agents, &review);
  assert!(
    agents.exists("pipeline-hub-flow-menu-own"),
    "Run on its own, inside a group"
  );
  assert_eq!(
    agents.text("pipeline-hub-flow-menu-down-detail"),
    "Nothing runs after it"
  );
}

#[test]
fn pipeline_reorder_move_up_from_the_menu_redraws_at_once_and_stores_nothing_until_save() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, ..]) = flow(&agents, false);
  open(&mut agents, id);
  let before = stored(&agents, id);

  open_menu(&mut agents, &plan);
  agents.click("pipeline-hub-flow-menu-up");
  agents.until("plan first", |s| {
    order(s, &[research.clone(), plan.clone()]) == [plan.clone(), research.clone()]
  });
  assert_eq!(stored(&agents, id), before, "nothing is stored until Save");

  // A step that is wired differently counts as a change the person allows, as every change of what a step waits for
  // does.
  agents.press_until("pipeline-hub-save", "the question", |s| {
    s.exists("pipeline-hub-confirm-confirm")
  });
  agents.press_until("pipeline-hub-confirm-confirm", "saved", |s| stored(s, id) != before);
  let after = stored(&agents, id);
  assert!(
    depends(&after, &research).contains(&plan.to_string()),
    "research now waits for plan"
  );
  assert!(depends(&after, &plan).is_empty());
}
