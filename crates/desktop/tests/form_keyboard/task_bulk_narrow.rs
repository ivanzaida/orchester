//! The task table and the scheduling review in the narrowest window (960): nothing is squeezed
//! to nothing or clipped.
use crate::{harness::Desktop, task_run::programs};

const NARROW: u32 = 960;

fn desktop() -> Desktop {
  Desktop::with_seeded_project(|repo| {
    let (_, fake) = programs();
    crate::task_run::seed(repo, &fake, false);
    super::drafts(repo);
  })
}

#[test]
fn task_bulk_narrow_table_keeps_the_title_and_leaves_out_the_pipeline() {
  let mut desktop = desktop();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-title-KON-1");
  assert!(desktop.tree.get_element_by_id("task-pipeline-KON-1").is_some());
  desktop.tree.resize(NARROW, 960);
  desktop.settle("the narrow table", |tree| {
    tree.get_element_by_id("task-pipeline-KON-1").is_none()
  });
  let title = desktop.bounds("task-title-KON-1");
  let row = desktop.bounds("task-row-KON-1");
  assert!(title.width >= 120.0, "the title keeps its room: {}", title.width);
  assert!(row.x + row.width <= NARROW as f32);
}

#[test]
fn task_bulk_narrow_review_puts_the_settings_under_the_order() {
  let mut desktop = desktop();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-select-KON-1");
  desktop.press("task-select-KON-1");
  desktop.press("task-select-KON-2");
  desktop.tree.resize(NARROW, 960);
  desktop.settle("the narrow list", |tree| {
    tree
      .get_element_by_id_mut("run-selected")
      .and_then(|button| button.bounds())
      .is_some()
  });
  desktop.press("run-selected");
  desktop.wait_for("tasks-review-settings");
  let order = desktop.bounds("tasks-review-order");
  let settings = desktop.bounds("tasks-review-settings");
  assert!(
    order.width >= 600.0,
    "the order takes the page's width: {}",
    order.width
  );
  assert!((settings.width - order.width).abs() < 1.0);
  assert!(
    settings.y >= order.y + order.height,
    "the settings come after the order"
  );
  let right = order.x + order.width;
  for id in [
    "tasks-review-order-task",
    "tasks-review-move-up",
    "tasks-review-move-down",
    "tasks-review-order-undo",
  ] {
    let control = desktop.bounds(id);
    assert!(control.x + control.width <= right + 0.5, "{id} fits the order column");
  }
  let sequential = desktop.bounds("tasks-review-sequential");
  assert!(sequential.x + sequential.width <= settings.x + settings.width + 0.5);
}
