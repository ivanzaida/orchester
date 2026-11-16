//! A version shown read-only offers none of the flow's reorder controls (PV2-06 with PV2-11): no handle, ⋯, keys,
//! *Add step* or *Edit steps*, and a card is not pressed. `cargo test -p orchester-app --test agents pipeline_reorder`.

use orchester_core::resource::StepKey;

use super::{
  pipeline_reorder::{alt, card, focus, hover},
  pipeline_step::keys,
  pipeline_versions::{open_list, two_versions},
};

#[test]
fn pipeline_reorder_a_version_shown_read_only_has_no_reorder_controls() {
  let (mut agents, ready) = two_versions();
  let [plan, build] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("the version", |s| s.exists("pipeline-hub-version-viewing"));
  assert!(agents.exists(&card(&plan)), "the version's flow is drawn");

  // Nothing is offered under the pointer or to the keys, and nothing moves.
  let route = agents.window.navigation().route_now();
  let (plan_y, build_y) = (y(&mut agents, &plan), y(&mut agents, &build));
  hover(&mut agents, &card(&plan));
  focus(&mut agents, &plan);
  alt(&mut agents, "ArrowDown");
  for id in [
    format!("{}-handle", card(&plan)),
    format!("{}-more", card(&plan)),
    "pipeline-hub-add-step".to_owned(),
    "pipeline-hub-edit-steps".to_owned(),
    "pipeline-hub-flow-menu".to_owned(),
  ] {
    assert!(!agents.exists(&id), "a read-only flow has no {id}");
  }
  assert_eq!((y(&mut agents, &plan), y(&mut agents, &build)), (plan_y, build_y));
  assert!(agents.exists("pipeline-hub-version-viewing"), "still the version");

  // A press opens nothing: the route stays.
  agents.click(&card(&plan));
  assert_eq!(
    agents.window.navigation().route_now(),
    route,
    "a card of a version is not pressed"
  );
  assert!(!agents.exists("pipeline-hub-step-head"));
}

fn y(agents: &mut super::harness::Agents, key: &StepKey) -> f32 {
  agents
    .window
    .tree
    .get_element_by_id_mut(&card(key))
    .and_then(|found| found.bounds())
    .expect("laid out")
    .y
}
