//! *Reply* under a question's answer on a run's review page (ORC-913): it opens the box inside the thread with
//! *Question* chosen, which asks Implement the new words in the same thread at once; *Comment* chosen keeps a comment
//! on the question's lines in the tray instead; *Ask again* and *Resolve* stay. On the QA seed's `review-page`
//! scenario, in a window whose project has no runner, so the follow-up is kept *Not asked*. Needs `git`, as the seed
//! does.
//! `cargo test -p orchester-app --test review_thread_reply thread_reply`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::time::Duration;

use lurq::app::events::ScrollPhase;
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};

use crate::{
  kontur::{SeedRequest, SeededTask, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long reading a run's diff with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);
/// How far one turn of the wheel scrolls the diff, and how many turns look for a thread.
const WHEEL_STEP: f32 = 150.0;
const WHEEL_TURNS: usize = 80;

struct Page {
  window: Window,
  _stand_ins: StandIns,
  project: ProjectId,
  tasks: Vec<SeededTask>,
}

impl Page {
  /// The `review-page` scenario in a window whose project has no runner: nothing reaches Implement.
  fn new() -> Self {
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
    Self {
      window,
      _stand_ins: stand_ins,
      project: seeded.project,
      tasks: seeded.tasks,
    }
  }

  /// Opens the review of the first run of "Sign webhook payloads", which holds answered questions.
  fn open(&mut self) {
    let task = self
      .tasks
      .iter()
      .find(|task| task.title == "Sign webhook payloads")
      .expect("the scenario seeds Sign webhook payloads");
    let key: TaskKey = task.key.parse().expect("a task key");
    let route = Route::Project(self.project, ProjectPage::TaskReview(key));
    self.window.navigation().go(route);
    self.window.until_within(GIT_WORK, "the review page", |window| {
      window.texts("review-status-text").is_some() && window.texts("review-diff").is_some()
    });
  }

  /// The id of the first thread whose *Ask again* shows inside the diff's view, turning the wheel over it: a row the
  /// list lays out past the view's edge cannot be pressed.
  fn answered_thread(&mut self) -> String {
    for _ in 0..WHEEL_TURNS {
      self
        .window
        .until("the rows in view", |window| window.texts("review-diff").is_some());
      let view = self
        .window
        .tree
        .get_element_by_id_mut("review-diff")
        .and_then(|element| element.bounds())
        .expect("the diff is laid out");
      let mut found = Vec::new();
      let mut stack = vec![self.window.tree.get_element_by_id("review-diff").expect("the diff")];
      while let Some(element) = stack.pop() {
        found.extend(
          element
            .id()
            .and_then(|id| id.strip_suffix("-ask-again"))
            .map(str::to_owned),
        );
        stack.extend(element.children());
      }
      for thread in found {
        let shown = self
          .window
          .tree
          .get_element_by_id_mut(&format!("{thread}-reply"))
          .and_then(|element| element.bounds())
          .is_some_and(|button| {
            let (x, y) = (button.x + button.width / 2.0, button.y + button.height / 2.0);
            (view.x..view.x + view.width).contains(&x) && (view.y..view.y + view.height).contains(&y)
          });
        if shown {
          return thread;
        }
      }
      let (x, y) = (view.x + view.width / 2.0, view.y + view.height / 2.0);
      self.window.tree.mouse_move(x, y);
      self.window.tree.scroll(x, y, 0.0, -WHEEL_STEP, ScrollPhase::Scroll);
      self.window.pass();
    }
    panic!("an answered question with its Reply");
  }

  /// Presses *Reply* of `thread` and waits for its box.
  fn reply(&mut self, thread: &str) {
    self.window.press(&format!("{thread}-reply"));
    self.window.until("the reply box", |window| {
      window.texts(&format!("{thread}-box")).is_some()
    });
  }
}

#[test]
fn thread_reply_offers_reply_beside_ask_again_and_resolve_under_an_answer() {
  let mut page = Page::new();
  page.open();
  let thread = page.answered_thread();
  assert_eq!(page.window.text(&format!("{thread}-reply")), "Reply");
  assert_eq!(page.window.text(&format!("{thread}-ask-again")), "Ask again");
  assert_eq!(page.window.text(&format!("{thread}-resolve")), "Resolve");
}

#[test]
fn thread_reply_opens_the_box_with_question_chosen() {
  let mut page = Page::new();
  page.open();
  let thread = page.answered_thread();
  page.reply(&thread);
  let send = page.window.text(&format!("{thread}-box-send"));
  assert_eq!(send, "Ask Implement");
  let hint = page.window.text(&format!("{thread}-box-hint"));
  assert!(hint.starts_with("Sent to Implement at once"), "{hint}");
  assert_eq!(page.window.text(&format!("{thread}-box-question")), "Question");
  assert_eq!(page.window.text(&format!("{thread}-box-comment")), "Comment");
}

#[test]
fn thread_reply_as_a_question_asks_implement_in_the_same_thread() {
  let mut page = Page::new();
  page.open();
  let thread = page.answered_thread();
  page.reply(&thread);
  page
    .window
    .set(&format!("{thread}-box-text"), "And what about retries?");
  page.window.press(&format!("{thread}-box-send"));
  page
    .window
    .until_within(GIT_WORK, "the follow-up in the thread", |window| {
      window.texts(&format!("{thread}-box")).is_none()
        && window
          .texts(&thread)
          .is_some_and(|words| words.join(" ").contains("And what about retries?"))
    });
  let words = page.window.text(&thread);
  assert!(
    words.contains("Not asked"),
    "with no runner the follow-up is kept not asked: {words}"
  );
  assert_eq!(
    page.window.text("review-tray-title"),
    "2 comments not sent",
    "a question never reaches the tray"
  );
}

#[test]
fn thread_reply_as_a_comment_keeps_a_comment_in_the_tray() {
  let mut page = Page::new();
  page.open();
  let thread = page.answered_thread();
  page.reply(&thread);
  page.window.press(&format!("{thread}-box-comment"));
  page.window.until("the comment chosen", |window| {
    window.texts(&format!("{thread}-box-send")) == Some(vec!["Add comment".to_owned()])
  });
  let hint = page.window.text(&format!("{thread}-box-hint"));
  assert!(hint.starts_with("Your reply waits in the tray"), "{hint}");
  page.window.set(&format!("{thread}-box-text"), "Cap it at three tries.");
  page.window.press(&format!("{thread}-box-send"));
  page.window.until_within(GIT_WORK, "the comment in the tray", |window| {
    window
      .texts("review-tray-title")
      .is_some_and(|title| title.join(" ") == "3 comments not sent")
  });
  assert!(
    page.window.texts(&format!("{thread}-box")).is_none(),
    "the box closed once the comment was kept"
  );
}

#[test]
fn thread_reply_cancel_closes_the_box_and_keeps_the_thread() {
  let mut page = Page::new();
  page.open();
  let thread = page.answered_thread();
  page.reply(&thread);
  page.window.press(&format!("{thread}-box-cancel"));
  page.window.until("the box closed", |window| {
    window.texts(&format!("{thread}-box")).is_none()
  });
  assert_eq!(page.window.text(&format!("{thread}-reply")), "Reply");
}
