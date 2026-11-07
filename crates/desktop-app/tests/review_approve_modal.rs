//! *Approve and merge* on a run's review page while comments wait unsent in the tray (ORC-801, `Review - v2` 09
//! `mhp3i`): it asks first in a modal whose title and words count the comments (naming the file and lines when it is
//! one), *Cancel* and Esc close it, *Send to Implement* sends the comments as the tray does and approves nothing, and
//! *Approve without them* discards them and approves. On the QA seed's `review-page` scenario, in a window whose
//! project has no runner. Needs `git`, as the seed does.
//! `cargo test -p orchester-app --test review_approve_modal approve_modal`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::{num::NonZeroU32, time::Duration};

use lurq::app::events::ScrollPhase;
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{id::ProjectId, project::TaskKey, run::RunNumber, time::Timestamp};

use crate::{
  kontur::{SeedRequest, SeededTask, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long reading a run's diff with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);
/// How long the merge takes: it reads the diff, makes the merge commit and moves the branch.
const MERGE_WORK: Duration = Duration::from_secs(180);

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
    window.tree.resize(1440, 900);
    Self {
      window,
      _stand_ins: stand_ins,
      project: seeded.project,
      tasks: seeded.tasks,
    }
  }

  /// Opens the review of run `number` of the task titled `title` at its route.
  fn open(&mut self, (title, number): (&str, u32)) {
    let task = self
      .tasks
      .iter()
      .find(|task| task.title == title)
      .unwrap_or_else(|| panic!("the scenario seeds {title}"));
    let key: TaskKey = task.key.parse().expect("a task key");
    let number = RunNumber::new(NonZeroU32::new(number).expect("runs are numbered from 1"));
    let route = Route::Project(self.project, ProjectPage::TaskReview(key.clone()));
    self.window.navigation().go(route);
    self.window.until_within(GIT_WORK, "the review page", |window| {
      window.texts("review-status-text").is_some() && window.texts("review-diff").is_some()
    });
    let crumbs = self.window.texts("review-crumbs").expect("the breadcrumb");
    assert_eq!(
      crumbs,
      ["Review", "/", &format!("{key} · run {}", number.get())],
      "{crumbs:?}"
    );
  }

  /// *Approve and merge* pressed with the two comments of the sender's and settings' lines not sent.
  fn ask(&mut self) {
    self.open(("Sign webhook payloads", 1));
    assert_eq!(self.window.text("review-tray-title"), "2 comments not sent");
    self.window.press("review-approve");
    self
      .window
      .until("the question", |window| window.texts("review-approve-ask").is_some());
  }

  fn escape(&mut self) {
    self
      .window
      .tree
      .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
    self
      .window
      .tree
      .key_up("Escape".to_owned(), "Escape".to_owned(), false, false, false);
    self.window.pass();
  }

  /// The id of the *Delete* of the first comment not sent that the diff lays out, turning the wheel over it.
  fn unsent_delete(&mut self) -> String {
    for _ in 0..12 {
      self
        .window
        .until("the rows in view", |window| window.texts("review-diff").is_some());
      let mut found = Vec::new();
      let mut stack = vec![self.window.tree.get_element_by_id("review-diff").expect("the diff")];
      while let Some(element) = stack.pop() {
        found.extend(
          element
            .id()
            .filter(|id| id.starts_with("review-thread-") && id.ends_with("-delete"))
            .map(str::to_owned),
        );
        stack.extend(element.children());
      }
      for id in found {
        if self
          .window
          .tree
          .get_element_by_id_mut(&id)
          .and_then(|element| element.bounds())
          .is_some()
        {
          return id;
        }
      }
      let bounds = self
        .window
        .tree
        .get_element_by_id_mut("review-diff")
        .and_then(|element| element.bounds())
        .expect("the diff is laid out");
      let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
      self.window.tree.mouse_move(x, y);
      self.window.tree.scroll(x, y, 0.0, -500.0, ScrollPhase::Scroll);
      self.window.pass();
    }
    panic!("a comment not sent with its Delete");
  }

  fn asking(&mut self) -> bool {
    self.window.texts("review-approve-ask").is_some()
  }
}

#[test]
fn approve_modal_asks_first_and_counts_the_comments_not_sent() {
  let mut page = Page::new();
  page.ask();
  assert_eq!(
    page.window.text("review-approve-ask-title"),
    "2 comments aren't sent yet"
  );
  let body = page.window.text("review-approve-ask-body");
  assert!(
    body.starts_with("Your 2 comments have not reached the agent."),
    "{body}"
  );
  assert_eq!(page.window.text("review-approve-ask-cancel"), "Cancel");
  assert_eq!(page.window.text("review-approve-ask-without"), "Approve without them");
  assert_eq!(page.window.text("review-approve-ask-send"), "Send to Implement");
  assert_eq!(
    page.window.text("review-tray-title"),
    "2 comments not sent",
    "asking changes nothing"
  );
}

#[test]
fn approve_modal_names_the_file_and_lines_of_the_one_comment_not_sent() {
  let mut page = Page::new();
  page.open(("Sign webhook payloads", 1));
  let delete = page.unsent_delete();
  page.window.press(&delete);
  page.window.until_within(GIT_WORK, "one comment left", |window| {
    window
      .texts("review-tray-title")
      .is_some_and(|title| title.join(" ") == "1 comment not sent")
  });
  page.window.press("review-approve");
  page
    .window
    .until("the question", |window| window.texts("review-approve-ask").is_some());
  assert_eq!(page.window.text("review-approve-ask-title"), "1 comment isn't sent yet");
  let body = page.window.text("review-approve-ask-body");
  assert!(
    body.starts_with("Your comment on src/") && body.contains(", line") && body.contains("has not reached the agent."),
    "{body}"
  );
  assert_eq!(page.window.text("review-approve-ask-without"), "Approve without it");
}

#[test]
fn approve_modal_cancel_and_esc_close_it_and_change_nothing() {
  let mut page = Page::new();
  page.ask();
  page.window.press("review-approve-ask-cancel");
  page.window.pass();
  assert!(!page.asking(), "Cancel closes it");
  assert_eq!(page.window.text("review-tray-title"), "2 comments not sent");
  page.window.press("review-approve");
  page
    .window
    .until("the question", |window| window.texts("review-approve-ask").is_some());
  page.window.pass();
  page.escape();
  page
    .window
    .until("Esc to close it", |window| window.texts("review-approve-ask").is_none());
  assert_eq!(page.window.text("review-tray-title"), "2 comments not sent");
  assert_eq!(page.window.text("review-status-text"), "Waiting for your review");
}

#[test]
fn approve_modal_send_to_implement_sends_the_comments_and_does_not_approve() {
  let mut page = Page::new();
  page.ask();
  page.window.press("review-approve-ask-send");
  // No runner takes them: they are kept not sent with why, as the tray's own *Send to Implement* does.
  page.window.until_within(GIT_WORK, "the send refused", |window| {
    window
      .texts("review-tray-title")
      .is_some_and(|title| title.join(" ") == "Not sent: the runner is not running")
  });
  assert!(!page.asking(), "it closed");
  assert_eq!(
    page.window.text("review-status-text"),
    "Waiting for your review",
    "nothing was approved"
  );
  assert!(page.window.texts("review-not-done").is_none(), "no approval was tried");
}

#[test]
fn approve_modal_approve_without_them_discards_the_comments_and_approves() {
  let mut page = Page::new();
  page.ask();
  page.window.press("review-approve-ask-without");
  page.window.until_within(GIT_WORK, "the comments discarded", |window| {
    window.texts("review-tray").is_none()
  });
  assert!(!page.asking(), "it closed");
  // The run merged: the page turns read-only, with no *Approve and merge* and no modal (ORC-800).
  page.window.until_within(MERGE_WORK, "the merge", |window| {
    window
      .texts("review-status-text")
      .is_some_and(|words| words.join(" ").starts_with("Merged into"))
  });
  assert!(
    page.window.texts("review-approve").is_none(),
    "Approve and merge is gone"
  );
  assert!(!page.asking(), "the modal stays closed");
}
