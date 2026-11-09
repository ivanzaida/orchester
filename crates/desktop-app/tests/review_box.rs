//! The review page's comment box keeps its place until its thread shows there (ORC-911), in the app's real window on
//! the QA seed's `review-page` scenario: between pressing *Add comment* or *Ask Implement* and the thread, every frame
//! holds the box with its words or the thread with them, never an empty place that makes the diff jump. Needs `git`,
//! as the seed does.
//! `cargo test -p orchester-app --test review_box review_box`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::time::Duration;

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{project::TaskKey, time::Timestamp};

use crate::{
  kontur::{SeedRequest, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long reading a run's diff with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);
/// The line the box is opened under: on the new side of the first file's diff.
const LINE: &str = "review-line-1-new-11";

/// The review of *Sign webhook payloads* in a window whose project has no runner, with the box open under [`LINE`].
fn page_with_box() -> (Window, StandIns) {
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
  window.press(LINE);
  window.until("the comment box", |window| window.texts("review-composer").is_some());
  (window, stand_ins)
}

/// Presses the box's send with `words` written, then lays the window out frame by frame until the box is gone and the
/// words stand in a thread, failing on any frame that has neither.
fn send_and_watch(window: &mut Window, words: &str) {
  window.set("review-composer-text", words);
  window.press("review-composer-send");
  let mut frames = 0_u32;
  window.until_within(GIT_WORK, "the thread in the box's place", |window| {
    frames += 1;
    let boxed = window.texts("review-composer").is_some();
    let threaded = window.text("review-diff").contains(words);
    // The words are also in the box, so the thread stands once the box is gone.
    assert!(
      boxed || threaded,
      "frame {frames}: the place under the line stood empty"
    );
    !boxed && threaded
  });
}

#[test]
fn review_box_comment_keeps_its_place_until_its_thread_shows() {
  let (mut window, _stand_ins) = page_with_box();
  send_and_watch(&mut window, "Name the budget's unit.");
  assert_eq!(
    window.text("review-tray-title"),
    "3 comments not sent",
    "the comment was kept unsent"
  );
}

#[test]
fn review_box_question_keeps_its_place_until_its_thread_shows() {
  let (mut window, _stand_ins) = page_with_box();
  window.press("review-composer-question");
  assert_eq!(window.text("review-composer-send"), "Ask Implement");
  send_and_watch(&mut window, "Why a budget and not a deadline?");
}
