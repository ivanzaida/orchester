//! The words on a run's review page select and copy like the Conversation's (ORC-912), in the app's real window on the
//! QA seed's `review-page` scenario: a drag over a thread's message selects its words and copies them whole, and does
//! not start the diff's drag over lines, so no comment box opens; the agent summary's sentence selects the same way.
//! Needs `git`, as the seed does.
//! `cargo test -p orchester-app --test review_select review_select`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::time::Duration;

use lurq::app::events::{MouseButton, ScrollPhase};
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{project::TaskKey, time::Timestamp};

use crate::{
  kontur::{SeedRequest, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long reading a run's diff with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);
/// How far one turn of the wheel scrolls the diff, and how many turns look for a thread's words.
const WHEEL_STEP: f32 = 500.0;
const WHEEL_TURNS: usize = 12;

/// The review page of the seed's first task, in a window whose project has no runner.
fn opened() -> (Window, StandIns) {
  let data = Data::new();
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: data.dir.path().join("repo-review-page"),
    scenario: scenarios::named("review-page").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = kontur::seed(&request).expect("the scenario seeds");
  let launch = data.launch_with(data.no_runner());
  let (mut window, stand_ins) = Window::launched_with_stand_ins(data, launch);
  window.tree.resize(1440, 1400);
  let task = seeded
    .tasks
    .iter()
    .find(|task| task.title == "Sign webhook payloads")
    .expect("the scenario seeds the task");
  let key: TaskKey = task.key.parse().expect("a task key");
  window
    .navigation()
    .go(Route::Project(seeded.project, ProjectPage::TaskReview(key)));
  window.until_within(GIT_WORK, "the review page", |window| {
    window.texts("review-status-text").is_some() && window.texts("review-diff").is_some()
  });
  (window, stand_ins)
}

/// The id of the first element under `within` whose id starts with `prefix` and ends with `suffix`.
fn find(window: &mut Window, within: &str, (prefix, suffix): (&str, &str)) -> Option<String> {
  let mut stack = vec![window.tree.get_element_by_id(within)?];
  while let Some(element) = stack.pop() {
    if let Some(id) = element.id().filter(|id| id.starts_with(prefix) && id.ends_with(suffix)) {
      return Some(id.to_owned());
    }
    stack.extend(element.children());
  }
  None
}

/// Turns the wheel over the diff until a thread's message words are laid out, and tells their element's id.
fn thread_words(window: &mut Window) -> String {
  for _ in 0..WHEEL_TURNS {
    window.pass();
    if let Some(id) = find(window, "review-diff", ("review-thread-", "-message-1-body")) {
      return id;
    }
    let bounds = window
      .tree
      .get_element_by_id_mut("review-diff")
      .and_then(|element| element.bounds())
      .expect("the diff is laid out");
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    window.tree.mouse_move(x, y);
    window.tree.scroll(x, y, 0.0, -WHEEL_STEP, ScrollPhase::Scroll);
  }
  panic!("a thread's second message in the diff");
}

/// Drags over the whole of the element `id`, from just inside its start to just inside its end.
fn drag_over(window: &mut Window, id: &str) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let (from, to) = (
    (bounds.x + 1.0, bounds.y + 2.0),
    (bounds.x + bounds.width - 2.0, bounds.y + bounds.height - 2.0),
  );
  window.tree.mouse_move(from.0, from.1);
  window.tree.mouse_down(from.0, from.1, MouseButton::Left);
  window.tree.mouse_move(to.0, to.1);
  window.tree.mouse_up(to.0, to.1, MouseButton::Left);
  window.pass();
}

#[test]
fn review_select_a_drag_over_a_threads_words_selects_them_and_opens_no_comment_box() {
  let (mut window, _stand_ins) = opened();
  let id = thread_words(&mut window);
  let words = window.text(&id);
  drag_over(&mut window, &id);
  let copied = window
    .tree
    .selected_text()
    .expect("the drag selects the thread's words");
  // An answer's Markdown (ORC-963) breaks its list items into lines where the element's texts join with spaces.
  assert_eq!(
    copied.split_whitespace().collect::<Vec<_>>(),
    words.split_whitespace().collect::<Vec<_>>(),
    "the words are copied whole"
  );
  assert!(
    window.texts("review-composer-text").is_none(),
    "a drag in a thread does not start the diff's selection of lines"
  );
}

#[test]
fn review_select_a_drag_over_the_agent_summary_selects_its_sentence() {
  let (mut window, _stand_ins) = opened();
  let words = window.text("review-summary-text");
  drag_over(&mut window, "review-summary-text");
  let copied = window.tree.selected_text().expect("the drag selects the summary");
  assert_eq!(copied.trim(), words.trim(), "the sentence is copied whole");
}
