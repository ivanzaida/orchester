//! The words a refused move says (PV2-06 round 3): a long reason is whole at any panel width, names the step once, goes
//! when the pointer and the keys leave the card it is under, and a read-only card is not a Tab stop. `cargo test -p
//! orchester-app --test agents pipeline_reorder`.

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, RepositoryAccess, StepKey},
};

use super::{
  harness::Agents,
  pipeline_flow::{add, agent, fails_back_to, open, store},
  pipeline_reorder::{alt, card, flow, focus},
  pipeline_step::keys,
  pipeline_versions::{open_list, two_versions},
};

const WORDS: &str = "pipeline-hub-flow-landing-words";
const LINE: &str = "pipeline-hub-flow-landing-parallel";

/// research → plan, plan returning to research, with names long enough that the refusal does not fit one line.
fn long(agents: &Agents) -> (ResourceId, [StepKey; 2], String) {
  let profile = Some(agents.profile(true));
  let kind = || agent(profile, RepositoryAccess::ReadOnly);
  let (first, second) = (
    "Research the whole of the dependency graph and the lockfile before anything is planned",
    "Plan the order the vendored manifests are reconciled with the lockfile entry by entry",
  );
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let research = add(&mut edit, first, &[], kind());
  let plan = add(&mut edit, second, &[&research], kind());
  fails_back_to(&mut edit, &plan, &research);
  let body = edit.validate().expect("a pipeline");
  let said = format!("Can't move {second}: it returns to 01 {first} on failure.");
  (store(agents, "Long names", &body, true), [research, plan], said)
}

/// Where an element stands: its left edge, top, width and height.
#[derive(Clone, Copy, Debug)]
struct Rect {
  x: f32,
  width: f32,
  height: f32,
}

impl Rect {
  fn right(self) -> f32 {
    self.x + self.width
  }
}

fn bounds(agents: &mut Agents, id: &str) -> Rect {
  let found = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|found| found.bounds())
    .unwrap_or_else(|| panic!("{id} laid out"));
  Rect {
    x: found.x,
    width: found.width,
    height: found.height,
  }
}

/// The refusal of Alt ↑ on the plan, with the window `width` wide: its text and where it stands.
fn refusal_at(agents: &mut Agents, plan: &StepKey, width: u32) -> (Rect, Rect) {
  agents.window.tree.resize(width, 900);
  agents.window.pass();
  focus(agents, plan);
  alt(agents, "ArrowUp");
  agents.until("the refusal", |s| s.exists(WORDS));
  agents.window.pass();
  (bounds(agents, WORDS), bounds(agents, "pipeline-hub-flow-frame"))
}

#[test]
fn pipeline_reorder_a_long_refusal_wraps_to_the_panels_width_and_is_whole() {
  let mut agents = Agents::new(true);
  let (id, [_, plan], said) = long(&agents);
  open(&mut agents, id);

  let (wide, wide_frame) = refusal_at(&mut agents, &plan, 1440);
  assert_eq!(agents.text(WORDS), said, "the whole reason, the moved step named once");
  assert!(
    wide.right() <= wide_frame.right() + 0.5,
    "inside the flow at 1440: {wide:?} in {wide_frame:?}"
  );

  let (narrow, narrow_frame) = refusal_at(&mut agents, &plan, 900);
  assert_eq!(agents.text(WORDS), said);
  assert!(
    narrow.right() <= narrow_frame.right() + 0.5,
    "nothing runs past the panel's edge at 900: {narrow:?} in {narrow_frame:?}"
  );
  assert!(
    narrow.height > wide.height + 1.0,
    "it takes more lines as the panel narrows (not cut): {} then {}",
    wide.height,
    narrow.height
  );
}

#[test]
fn pipeline_reorder_a_refusal_goes_when_the_pointer_and_the_keys_leave_its_card() {
  let mut agents = Agents::new(true);
  let (id, [research, plan, ..]) = flow(&agents, true);
  open(&mut agents, id);

  focus(&mut agents, &plan);
  alt(&mut agents, "ArrowUp");
  agents.until("the refusal", |s| s.exists(LINE));

  // The keys are still on the card: it stays. They go to another: it goes.
  agents.window.pass();
  assert!(agents.exists(LINE), "it stays while the card has the keys");
  focus(&mut agents, &research);
  agents.until("gone", |s| !s.exists(LINE));
  assert!(agents.exists(&card(&plan)));
}

#[test]
fn pipeline_reorder_a_read_only_card_is_not_a_tab_stop_and_an_editable_one_is() {
  let (mut agents, ready) = two_versions();
  let [plan, _] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  let tabbed = |agents: &mut Agents| -> Vec<String> {
    let mut seen = Vec::new();
    for _ in 0..80 {
      agents
        .window
        .tree
        .key_down("Tab".to_owned(), "Tab".to_owned(), false, false, false);
      agents.window.pass();
      if let Some(id) = agents
        .window
        .tree
        .focused_element()
        .and_then(|element| element.id().map(str::to_owned))
      {
        seen.push(id);
      }
    }
    seen
  };

  let editable = tabbed(&mut agents);
  assert!(
    editable.contains(&card(&plan)),
    "the editable flow's cards are Tab stops: {editable:?}"
  );

  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("the version", |s| s.exists("pipeline-hub-version-viewing"));
  assert!(agents.exists(&card(&plan)));
  let read_only = tabbed(&mut agents);
  assert!(
    !read_only.iter().any(|id| id.starts_with("pipeline-hub-flow-step-")),
    "a version's cards are not Tab stops: {read_only:?}"
  );
}
