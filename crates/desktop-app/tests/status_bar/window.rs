//! The bar in the real window, its actions going through the window's own: *Stop* on the QA seed's
//! `tasks` scenario with one run auto-approving.

use orchester_core::time::Timestamp;
use orchester_workspace::tasks::{
  data::ProjectTasks,
  run_approvals::{RunApprovalChange, apply_in},
};

use super::{
  MouseButton,
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window},
};

#[test]
fn status_stop_in_the_window_ends_auto_approval() {
  let data = Data::new();
  let repo = data.dir.path().join("repo-tasks");
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: repo.clone(),
    scenario: scenarios::named("tasks").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  kontur::seed(&request).expect("the scenario seeds");
  let tasks = ProjectTasks::load(&repo).expect("the tasks read");
  let running = tasks
    .tasks
    .iter()
    .find(|task| task.key().to_string() == "KON-16")
    .and_then(|task| tasks.phases.get(&task.id())?.run())
    .expect("KON-16 runs")
    .id;
  apply_in(&repo, running, RunApprovalChange::AutoApprove(true)).expect("the run auto-approves");
  assert_eq!(
    ProjectTasks::load(&repo).expect("the tasks read").auto_approving.len(),
    1
  );
  let launch = data.launch();
  let mut window = Window::of(data, launch);
  window.until("Stop to show", |window| {
    window.texts("status-auto-approving-stop").is_some()
  });
  let bounds = window
    .tree
    .get_element_by_id_mut("status-auto-approving-stop")
    .and_then(|element| element.bounds())
    .expect("Stop is laid out");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
  window.until("the run to stop auto-approving", |window| {
    window.texts("status-auto-approving").is_none()
  });
  assert!(
    ProjectTasks::load(&repo)
      .expect("the tasks read")
      .auto_approving
      .is_empty(),
    "the run's auto-approval is off in its data"
  );
}
