//! A run's transcript growing reads again only what shows that run's transcript, and a burst of lines at most a
//! few times; a run changing state reads again what shows runs.

use orchester_app::routes::{ProjectPage, TaskView};

use super::Counted;

/// `key`'s page on `view`, its reads done.
fn task_page(scenario: &str, key: &str, view: TaskView, shown: &str) -> Counted {
  let mut counted = Counted::open(scenario);
  counted.show(ProjectPage::Task(key.parse().expect("a task key"), view), shown);
  counted
}

#[test]
fn reads_measure_a_transcript_line() {
  for (key, view, shown) in [
    ("KON-12", TaskView::Conversation, "conversation-chat"),
    ("KON-12", TaskView::Summary, "task-summary"),
    ("KON-1", TaskView::Summary, "task-summary"),
  ] {
    let mut counted = task_page("conversation", key, view.clone(), shown);
    counted.report(false);
    let line = counted.settle();
    counted.report(true);
    let state = counted.settle();
    for _ in 0..20 {
      counted.report(false);
      counted.window.tree.tick_futures();
      counted.window.pass();
      std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let burst = counted.settle();
    eprintln!("{key} {view:?}: line {line:?}; state {state:?}; burst of 20 {burst:?}");
  }
}
