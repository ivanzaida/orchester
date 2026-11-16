//! Alt ↑ on the first card and Alt ↓ on the last change nothing, and say why under the card as a refused drop does
//! (N-013): the words of the menu's faded *Move up* and *Move down*. `cargo test -p orchester-app --test agents
//! pipeline_reorder_edges`.

use orchester_core::resource::{Pipeline, PipelineEdit, RepositoryAccess};

use super::{
  harness::Agents,
  pipeline_flow::{add, agent, open, store},
  pipeline_reorder::{alt, focus, stored},
};

/// What a refusal under a card says.
const REFUSAL: &str = "pipeline-hub-flow-landing-parallel";

#[test]
fn pipeline_reorder_edges_alt_up_on_the_first_step_and_alt_down_on_the_last_say_why() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  // A line: Build waits for Plan.
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let plan = add(&mut edit, "Plan", &[], agent(Some(profile), RepositoryAccess::ReadOnly));
  let build = add(
    &mut edit,
    "Build",
    &[&plan],
    agent(Some(profile), RepositoryAccess::ReadOnly),
  );
  let id = store(&agents, "Two steps", &edit.validate().expect("a pipeline"));
  let before = stored(&agents, id);
  open(&mut agents, id);

  focus(&mut agents, &plan);
  alt(&mut agents, "ArrowUp");
  agents.until("the refusal under Plan", |s| s.exists(REFUSAL));
  assert_eq!(agents.text(REFUSAL), "Can't move Plan: nothing runs before it.");

  focus(&mut agents, &build);
  alt(&mut agents, "ArrowDown");
  agents.until("the refusal under Build", |s| {
    s.exists(REFUSAL) && s.text(REFUSAL) == "Can't move Build: nothing runs after it."
  });
  assert!(
    !agents
      .text(&format!("pipeline-hub-row-{id}-line"))
      .starts_with("Unsaved"),
    "a refused move holds no change"
  );
  assert_eq!(stored(&agents, id), before);

  // A move that changes the steps takes the refusal away.
  alt(&mut agents, "ArrowUp");
  agents.until("moved, and nothing said", |s| !s.exists(REFUSAL));
}
