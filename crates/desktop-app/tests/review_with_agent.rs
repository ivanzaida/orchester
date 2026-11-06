//! A task's review page while it is with the agent (ORC-799, `Review - v2` 06 `gwE2q`), in the app's real window on the
//! QA seed's `review-page` scenario (`examples/seed`): *Delivery log per endpoint*'s run went back to Implement with a
//! comment sent. The status says *With the agent* and what Implement works on, *Approve and merge* is not shown, the
//! sent comment's tag says when it was sent, and under the diff the agent bar stands in the tray's place, with *Open
//! conversation* leading to the task's Conversation. Comments written meanwhile wait unsent in the tray above the bar
//! and cannot be sent until the task is back. Needs `git`, as the seed does.
//! `cargo test -p orchester-app --test review_with_agent review_with_agent`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::time::Duration;

use orchester_app::routes::{ProjectPage, Route, TaskTab, TaskView};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};

use crate::{
  kontur::{SeedRequest, SeededTask, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long reading a run's diff with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);

struct Page {
  window: Window,
  _stand_ins: StandIns,
  project: ProjectId,
  key: TaskKey,
}

impl Page {
  /// The review of *Delivery log per endpoint* in a window whose project has no runner.
  fn open() -> Self {
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
    let task: &SeededTask = seeded
      .tasks
      .iter()
      .find(|task| task.title == "Delivery log per endpoint")
      .expect("the scenario seeds a task with the agent");
    let key: TaskKey = task.key.parse().expect("a task key");
    let mut page = Self {
      window,
      _stand_ins: stand_ins,
      project: seeded.project,
      key: key.clone(),
    };
    page
      .window
      .navigation()
      .go(Route::Project(page.project, ProjectPage::TaskReview(key)));
    page.window.until_within(GIT_WORK, "the review page", |window| {
      window.texts("review-status-text").is_some() && window.texts("review-diff").is_some()
    });
    page
  }

  /// The top of the element with the id `id`, laid out.
  fn top(&mut self, id: &str) -> f32 {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
      .y
  }
}

/// Whether `words` read `prefix` and a clock time: *Sent 12:05*.
fn says_when(words: &str, prefix: &str) -> bool {
  words.find(prefix).is_some_and(|at| {
    let time = &words[at + prefix.len()..];
    time.len() >= 5 && time.as_bytes()[2] == b':' && time[..5].chars().filter(char::is_ascii_digit).count() == 4
  })
}

#[test]
fn review_with_agent_says_what_implement_works_on_and_shows_no_approve() {
  let mut page = Page::open();
  assert_eq!(page.window.text("review-status-text"), "With the agent");
  let meta = page.window.text("review-status-meta");
  assert!(
    meta.starts_with("Implement is working on your comment · sent ") && says_when(&meta, "sent "),
    "{meta}"
  );
  assert!(page.window.texts("review-approve").is_none(), "Approve and merge waits");
  assert!(page.window.texts("review-approve-reason").is_none());
  assert!(page.window.texts("review-open-task").is_some(), "Open task stays");
  let diff = page.window.text("review-diff");
  assert!(
    says_when(&diff, "Sent "),
    "the comment's tag says when it was sent: {diff}"
  );
}

#[test]
fn review_with_agent_bar_stands_in_the_trays_place_and_opens_the_conversation() {
  let mut page = Page::open();
  assert_eq!(
    page.window.text("review-agent-title"),
    "Implement is working on 1 comment"
  );
  assert_eq!(
    page.window.text("review-agent-body"),
    "The task comes back here when it finishes; new comments wait until then."
  );
  assert_eq!(page.window.text("review-agent-open"), "Open conversation");
  assert!(page.window.texts("review-tray").is_none(), "no comment waits: no tray");
  page.window.press("review-agent-open");
  let route = Route::Project(
    page.project,
    ProjectPage::Task(page.key.clone(), TaskView::of(TaskTab::Conversation)),
  );
  page
    .window
    .until("the Conversation", |window| window.navigation().route_now() == route);
}

#[test]
fn review_with_agent_a_new_comment_waits_in_the_tray_above_the_bar_and_is_not_sent() {
  let mut page = Page::open();
  let line = first_line(&mut page);
  page.window.press(&line);
  page
    .window
    .until("the comment box", |window| window.texts("review-composer").is_some());
  page.window.set("review-composer-text", "Name the log's unit.");
  page.window.press("review-composer-send");
  page.window.until_within(GIT_WORK, "the comment in the tray", |window| {
    window.texts("review-tray-title").is_some()
  });
  assert_eq!(page.window.text("review-tray-title"), "1 comment not sent");
  assert_eq!(
    page.window.text("review-tray-detail"),
    "Sent once the task is back from Implement."
  );
  assert_eq!(page.window.text("review-tray-send"), "Send to Implement (1)");
  assert_eq!(
    page.window.text("review-agent-title"),
    "Implement is working on 1 comment",
    "the earlier batch alone"
  );
  let (tray, bar) = (page.top("review-tray"), page.top("review-agent"));
  assert!(tray < bar, "the tray {tray} shows above the bar {bar}");
  page.window.press("review-tray-send");
  page.window.pass();
  assert_eq!(
    page.window.text("review-tray-title"),
    "1 comment not sent",
    "pressing Send changes nothing while the agent works"
  );
  assert!(page.window.text("review-diff").contains("Name the log's unit."));
}

/// The id of the first added line of the diff's first file.
fn first_line(page: &mut Page) -> String {
  let mut stack = vec![page.window.tree.get_element_by_id("review-diff").expect("the diff")];
  while let Some(element) = stack.pop() {
    if let Some(id) = element
      .id()
      .filter(|id| id.starts_with("review-line-") && id.contains("-new-"))
    {
      return id.to_owned();
    }
    let mut children: Vec<_> = element.children().into_iter().collect();
    children.reverse();
    stack.extend(children);
  }
  panic!("a line to comment on");
}
