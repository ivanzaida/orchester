//! One count of what waits for the person wherever it shows (`ProjectTasks::needs_you`): the pill, the panel, the
//! Overview's Needs you tab (`BoardTab::count`) and All projects say the same number of one project.

use orchester_app::{project::TasksState, routes::Route};
use orchester_workspace::tasks::board::BoardTab;

use super::{Seeded, Window, open, seeded, text};

/// The Overview's Needs you tab's count, of the project's tasks as they stand with its runner.
fn overview_count(window: &Window, seeded: &Seeded) -> usize {
  match window.projects().feed(seeded.project).tasks_with_runner(&seeded.repo) {
    TasksState::Ready(tasks) => BoardTab::NeedsYou.count(&tasks),
    state => panic!("the tasks are read: {state:?}"),
  }
}

#[test]
fn needs_you_panel_counts_as_the_pill_the_overview_tab_and_all_projects() {
  let (mut window, seeded) = seeded("tasks");
  assert_eq!(window.texts("needs-you-pill"), Some(vec!["6 need you".to_owned()]));
  open(&mut window);
  assert_eq!(text(&mut window, "needs-you-count"), "6");
  assert_eq!(overview_count(&window, &seeded), 6);
  window.navigation().go(Route::AllProjects);
  let cell = format!("project-row-needs-you-{}", seeded.project);
  window.until("All projects' counts to be read", |window| {
    window.texts(&cell).is_some_and(|texts| texts.join(" ") != "—")
  });
  assert_eq!(text(&mut window, &cell), "6");
}
