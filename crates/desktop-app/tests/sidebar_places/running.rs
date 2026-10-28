//! Running shows only while runs go: a project without runs under way has no Running section; one with runs shows
//! the section with how many there are, a row each.

use super::{
  overview, seeded,
  support::{Window, project_id},
};

#[test]
fn sidebar_running_shows_only_while_runs_go() {
  let mut quiet = Window::open();
  quiet.navigation().go(overview(project_id()));
  quiet.until("the tasks to be read", |window| {
    window.texts("nav-tasks") == Some(vec!["Tasks".to_owned()])
  });
  assert!(quiet.texts("running").is_none(), "no run under way, no Running");
  let (mut busy, _) = seeded("tasks");
  busy.until("the runs under way", |window| window.texts("running").is_some());
  let texts = busy.texts("running").expect("Running shows");
  assert_eq!(texts[0], "Running");
  let count: usize = busy.text("running-count").parse().expect("a count");
  assert!(count > 0, "{texts:?}");
  assert_eq!(
    texts.iter().filter(|text| text.starts_with("KON-")).count(),
    count,
    "a row per run under way: {texts:?}"
  );
}
