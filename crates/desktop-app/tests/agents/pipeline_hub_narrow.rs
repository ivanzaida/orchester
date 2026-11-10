//! The Pipelines hub in a window narrower than its board (PV2-17): under the `xl` breakpoint the list gives way first,
//! and the panel keeps a width its rows, cards and bar are read at, inside the page.
//! `cargo test -p orchester-app --test agents pipeline_hub_narrow`.

use orchester_app::routes::PipelinesFilter;

use super::{
  harness::Agents,
  pipeline_hub::{open, setup},
};

/// The left edge and the width of `id`.
fn span(agents: &mut Agents, id: &str) -> (f32, f32) {
  let found = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|found| found.bounds())
    .unwrap_or_else(|| panic!("{id} laid out"));
  (found.x, found.width)
}

#[test]
fn pipeline_hub_narrow_the_list_gives_way_before_the_panel() {
  let (mut agents, [ready, ..]) = setup();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists("pipeline-hub-panel"));
  let (_, wide) = span(&mut agents, "pipeline-hub-list-column");
  assert!((wide - 440.0).abs() < 1.0, "the board's list at 1440: {wide}");

  agents.window.tree.resize(1100, 900);
  agents.until("laid out again", |s| span(s, "pipeline-hub-list-column").1 < 440.0);
  let (_, list) = span(&mut agents, "pipeline-hub-list-column");
  let (panel_x, panel) = span(&mut agents, "pipeline-hub-panel");
  let (frame_x, frame) = span(&mut agents, "pipeline-hub-master-detail");
  assert!((list - 260.0).abs() < 1.0, "the list gives way: {list}");
  assert!(panel >= 500.0, "the panel keeps a width it is read at: {panel}");
  assert!(
    panel_x + panel <= frame_x + frame + 0.5,
    "the panel stays inside the page: {panel_x}+{panel} in {frame_x}+{frame}"
  );
}
