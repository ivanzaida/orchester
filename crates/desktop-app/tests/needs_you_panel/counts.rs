//! One count of what waits for the person wherever it shows (`ProjectTasks::needs_you`): the pill, the panel, the
//! Overview's Needs you tab (`OverviewTab::count`) and All projects say the same number of one project, and none of
//! them counts a task whose workflow status is Done.

use orchester_app::{project::TasksState, routes::Route};
use orchester_core::{
  store::TaskStore,
  task::organization::{ChangeActor, WorkflowStatus},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::overview::OverviewTab;

use super::{Seeded, Window, open, seeded_with, text};

/// Marks the task titled `title`, which stopped at its limit and waits for it to be raised, Done.
fn finish(seeded: &Seeded, title: &str) {
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's database");
  let task = db
    .tasks()
    .expect("the project's tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == title)
    .expect("the seeded task");
  db.update_task_organization(task.id(), ChangeActor::Person, Timestamp::now(), |organization| {
    organization.with_status(WorkflowStatus::Done)
  })
  .expect("the task is Done");
}

/// The Overview's Needs you tab's count, of the project's tasks as they stand with its runner.
fn overview_count(window: &Window, seeded: &Seeded) -> usize {
  match window.projects().feed(seeded.project).tasks_with_runner(&seeded.repo) {
    TasksState::Ready(tasks) => OverviewTab::NeedsYou.count(&tasks),
    state => panic!("the tasks are read: {state:?}"),
  }
}

#[test]
fn needs_you_panel_counts_as_the_pill_the_overview_tab_and_all_projects() {
  let finished = "Explain an expired GitHub token in the status bar";
  let (mut window, seeded) = seeded_with("tasks", |seeded| finish(seeded, finished));
  assert_eq!(
    window.texts("needs-you-pill"),
    Some(vec!["5 need you".to_owned()]),
    "the seed's six, less the Done task's limit"
  );
  open(&mut window);
  assert_eq!(text(&mut window, "needs-you-count"), "5");
  assert_eq!(overview_count(&window, &seeded), 5);
  window.navigation().go(Route::AllProjects);
  let cell = format!("project-row-needs-you-{}", seeded.project);
  window.until("All projects' counts to be read", |window| {
    window.texts(&cell).is_some_and(|texts| texts.join(" ") != "—")
  });
  assert_eq!(text(&mut window, &cell), "5");
}
