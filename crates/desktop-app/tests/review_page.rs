//! A run's review page (ORC-697, `Review · redesign` 09i `wLyFt` and the states of `RBR9O`) in the app's real window,
//! on the QA seed's `review-page` scenario (`examples/seed`): opened from its *Waiting for your review* row at its
//! task's review route, with the run's real diff, its files and *Viewed* marks, every thread state, the comment
//! box that adds a comment to the tray and whose field takes the focus as it opens, the **+** only on the line under
//! the pointer and never over its numbers (ORC-702), a send the runner did not take kept with why, a send QA holds
//! (`hold_hand_offs`) shown *Sending…* in its threads and the tray, and *Approve and
//! merge* unavailable while Implement works on a comment sent, whose link names the step at work; on
//! `review-changed`, a branch that moved after the review
//! (ORC-699) told with its new commits, and *Approve and merge* unavailable until they are taken in; on
//! `review-approved`, a task a person approved (ORC-800), read-only. Needs `git`, as the seed does.
//! `cargo test -p orchester-app --test review_page review_page`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::{num::NonZeroU32, time::Duration};

use lurq::app::events::ScrollPhase;
use orchester_app::{
  mcp::tools::hold_hand_offs,
  routes::{ProjectPage, Route},
};
use orchester_core::{id::ProjectId, project::TaskKey, run::RunNumber, time::Timestamp};
use serde_json::json;

use crate::{
  kontur::{SeedRequest, SeededTask, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long reading a run's diff with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);
/// How far one turn of the wheel scrolls the diff, and how many turns read it to its end.
const WHEEL_STEP: f32 = 500.0;
const WHEEL_TURNS: usize = 12;

struct Page {
  window: Window,
  _stand_ins: StandIns,
  project: ProjectId,
  tasks: Vec<SeededTask>,
}

impl Page {
  /// The scenario in a window whose project has no runner: answers under way stay, and nothing reaches Implement.
  fn new() -> Self {
    Self::of("review-page")
  }

  /// The seed's `scenario` in a window whose project has no runner.
  fn of(scenario: &str) -> Self {
    let data = Data::new();
    let repo = data.dir.path().join(format!("repo-{scenario}"));
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo,
      scenario: scenarios::named(scenario).expect("a known scenario"),
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

  /// Opens the review of run `number` of the task titled `title` at its route; runs are numbered in the project, in
  /// the order the scenario seeds them.
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
    self.until_read();
    let crumbs = self.window.texts("review-crumbs").expect("the breadcrumb");
    assert_eq!(
      crumbs,
      ["Review", "/", &format!("{key} · run {}", number.get())],
      "the task's latest run in review: {crumbs:?}"
    );
  }

  /// The diff's words from its top to its end, turning the wheel over it: its rows are laid out only where they show.
  fn whole_diff(&mut self) -> String {
    let mut seen = String::new();
    for _ in 0..WHEEL_TURNS {
      self
        .window
        .until("the rows in view", |window| window.texts("review-diff").is_some());
      seen.push_str(&self.diff_text());
      seen.push(' ');
      let bounds = self
        .window
        .tree
        .get_element_by_id_mut("review-diff")
        .and_then(|element| element.bounds())
        .expect("the diff is laid out");
      let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
      self.window.tree.mouse_move(x, y);
      self.window.tree.scroll(x, y, 0.0, -WHEEL_STEP, ScrollPhase::Scroll);
      self.window.pass();
    }
    seen
  }

  fn until_read(&mut self) {
    self.window.until_within(GIT_WORK, "the review page", |window| {
      window.texts("review-status-text").is_some() && window.texts("review-diff").is_some()
    });
  }

  fn diff_text(&mut self) -> String {
    self.window.text("review-diff")
  }
}

#[test]
fn review_page_opens_from_its_waiting_row_with_the_runs_real_diff() {
  let mut page = Page::new();
  page
    .window
    .navigation()
    .go(Route::Project(page.project, ProjectPage::Merges));
  page.window.until_within(GIT_WORK, "the merges", |window| {
    window.texts("merges-waiting-rows").is_some()
  });
  let rows: Vec<String> = page
    .window
    .tree
    .get_element_by_id("merges-waiting-rows")
    .map_or_else(Vec::new, |rows| {
      rows
        .children()
        .into_iter()
        .filter_map(|row| row.id().map(str::to_owned))
        .collect()
    });
  let row = rows
    .into_iter()
    .find(|row| page.window.text(row).contains("Sign webhook payloads"))
    .expect("the run waits for review");
  page.window.press(&format!("{row}-open"));
  page.until_read();
  let route = page.window.navigation().route_now();
  assert!(
    matches!(route, Route::Project(_, ProjectPage::TaskReview(_))),
    "{route:?}"
  );
  assert_eq!(page.window.text("review-status-text"), "Waiting for your review");
  let meta = page.window.text("review-status-meta");
  assert!(
    meta.contains("run #1") && meta.contains("4 files") && meta.contains("head "),
    "{meta}"
  );
  assert_eq!(page.window.text("review-files-viewed"), "1 of 4 viewed");
  assert_eq!(page.window.text("merge-attempt-title"), "Sign webhook payloads");
  let diff = page.diff_text();
  assert!(diff.contains("src/webhooks/sender.rs"), "{diff}");
  assert!(
    diff.contains("let budget = endpoint.settings.retry_budget;"),
    "the real diff: {diff}"
  );
  assert!(
    page.window.texts("review-file-0-fold").is_some(),
    "the viewed migration shows its head"
  );
}

#[test]
fn review_page_shows_every_thread_state_and_the_idle_tray() {
  let mut page = Page::new();
  page.open(("Sign webhook payloads", 1));
  // The comment not sent on the sender's lines and the outdated one on the settings' wait in the tray.
  assert_eq!(page.window.text("review-tray-title"), "2 comments not sent");
  assert_eq!(page.window.text("review-tray-send"), "Send to Implement (2)");
  let diff = page.whole_diff();
  for state in [
    "Not sent",
    "Sent to Implement",
    "Starting…",
    "Implement is answering",
    "Stop answering",
    "Stopped by you",
    "Answer stopped.",
    "Answered · no code changed",
    "Not asked",
    "Ask again",
    "Addressed in",
    "Reply",
    "Resolved",
    "1 outdated",
  ] {
    assert!(diff.contains(state), "{state} in {diff}");
  }
}

#[test]
fn review_page_comment_box_adds_a_comment_to_the_tray() {
  let mut page = Page::new();
  page.open(("Sign webhook payloads", 1));
  let line = first_line(&mut page, "review-line-1-new-");
  page.window.press(&line);
  page
    .window
    .until("the comment box", |window| window.texts("review-composer").is_some());
  let send = page.window.text("review-composer-send");
  assert_eq!(send, "Add comment");
  page.window.set("review-composer-text", "Name the budget's unit.");
  page.window.press("review-composer-send");
  page.window.until_within(GIT_WORK, "the comment in the tray", |window| {
    window
      .texts("review-tray-title")
      .is_some_and(|title| title.join(" ") == "3 comments not sent")
  });
  assert!(
    page.window.texts("review-composer").is_none(),
    "the box closed once the comment was kept"
  );
  assert!(page.diff_text().contains("Name the budget's unit."));
}

#[test]
fn review_page_send_the_runner_did_not_take_keeps_the_comments_with_why() {
  let mut page = Page::new();
  page.open(("Per-endpoint secrets", 2));
  let title = page.window.text("review-tray-title");
  assert_eq!(title, "Not sent: the runner is not running");
  assert_eq!(page.window.text("review-tray-send"), "Try again");
  assert!(
    page
      .window
      .text("review-tray-detail")
      .starts_with("The send failed; nothing is lost.")
  );
}

/// QA's `hold_hand_offs` keeps a send under way: every comment it sends and the tray say *Sending…* until it is let go,
/// and then, with no runner, the comments wait unsent again with why.
#[test]
fn review_page_send_held_shows_sending_until_let_go() {
  let mut page = Page::new();
  page.open(("Sign webhook payloads", 1));
  let hand_offs = page
    .window
    .launch
    .services()
    .expect("a started session")
    .hand_offs
    .clone();
  hold_hand_offs(&hand_offs, &json!({ "held": true })).expect("a flag");
  page.window.press("review-tray-send");
  page.window.until_within(GIT_WORK, "the comments sending", |window| {
    window
      .texts("review-tray-title")
      .is_some_and(|title| title.join(" ") == "Sending 2 comments to Implement…")
  });
  assert_eq!(page.window.text("review-tray-send"), "Sending…");
  assert!(page.diff_text().contains("Sending…"), "the comment's own tag");
  hold_hand_offs(&hand_offs, &json!({ "held": false })).expect("a flag");
  page.window.until_within(GIT_WORK, "the send refused", |window| {
    window
      .texts("review-tray-title")
      .is_some_and(|title| title.join(" ") == "Not sent: the runner is not running")
  });
}

#[test]
fn review_page_approve_is_unavailable_while_implement_works_on_a_comment_sent() {
  let mut page = Page::new();
  page.open(("Delivery log per endpoint", 3));
  assert_eq!(page.window.text("review-status-text"), "Implement is changing the code");
  let reason = page.window.text("review-approve-reason");
  assert!(reason.contains("Implement is at work"), "{reason}");
  let diff = page.diff_text();
  assert!(diff.contains("Sent · Implement is changing the code"), "{diff}");
  assert!(
    diff.contains("In the Conversation · run #3, step 03 Implement"),
    "the step Implement works in: {diff}"
  );
}

#[test]
fn review_page_comment_box_field_takes_the_focus_as_it_opens() {
  let mut page = Page::new();
  page.open(("Sign webhook payloads", 1));
  page.window.press("review-line-1-new-11");
  page
    .window
    .until("the comment box", |window| window.texts("review-composer").is_some());
  page.window.pass();
  assert_eq!(
    page.window.tree.focused_element().and_then(|element| element.id()),
    Some("review-composer-text"),
    "what is typed next goes into the box"
  );
}

#[test]
fn review_page_shows_the_plus_only_on_the_line_under_the_pointer_beside_its_numbers() {
  let mut page = Page::new();
  page.open(("Sign webhook payloads", 1));
  // An added line has no old number: the + stands in its column.
  page.window.hover("review-line-1-new-11");
  assert_eq!(count(&mut page, "review-diff", "review-line-add"), 1);
  assert_eq!(count(&mut page, "review-line-1-new-11", "review-line-add"), 1);
  // An unchanged line keeps both its numbers, the + in its sign's place; the line before lets go of its +.
  page.window.hover("review-line-1-new-8");
  assert_eq!(count(&mut page, "review-diff", "review-line-add"), 1);
  assert_eq!(count(&mut page, "review-line-1-new-8", "review-line-add"), 1);
  let numbers = page.window.texts("review-line-1-new-8").expect("the line");
  assert_eq!(
    numbers.get(..2),
    Some(&["8".to_owned(), "8".to_owned()][..]),
    "{numbers:?}"
  );
}

#[test]
fn review_page_tells_the_commits_after_the_review_and_approves_only_once_they_are_taken_in() {
  let mut page = Page::of("review-changed");
  page.open(("Sign webhook payloads", 1));
  assert_eq!(page.window.text("review-status-text"), "Changed since your review");
  let notice = page.window.text("review-changed");
  assert!(notice.contains("2 new commits"), "{notice}");
  let commits = page.window.text("review-changed-commits");
  let (newest, oldest) = (
    commits.find("Test the 30 s bound"),
    commits.find("Cap the delay and its jitter together"),
  );
  assert!(newest.is_some() && oldest.is_some() && newest < oldest, "{commits}");
  let reason = page.window.text("review-approve-reason");
  assert!(reason.contains("review the new commits first"), "{reason}");
  page.window.press("review-changed-take");
  page.window.until("the commits taken in", |window| {
    window.texts("review-changed").is_none() && window.texts("review-approve-reason").is_none()
  });
  assert_eq!(page.window.text("review-status-text"), "Waiting for your review");
}

/// The id of the first line of the diff whose id starts with `prefix`.
fn first_line(page: &mut Page, prefix: &str) -> String {
  let mut found = None;
  let mut stack = vec![page.window.tree.get_element_by_id("review-diff").expect("the diff")];
  while let Some(element) = stack.pop() {
    if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
      found = Some(id.to_owned());
      break;
    }
    let mut children: Vec<_> = element.children().into_iter().collect();
    children.reverse();
    stack.extend(children);
  }
  found.unwrap_or_else(|| panic!("a line {prefix}…"))
}

/// How many elements inside the element `within` have the id `id`.
fn count(page: &mut Page, within: &str, id: &str) -> usize {
  let mut found = 0;
  let mut stack = vec![
    page
      .window
      .tree
      .get_element_by_id(within)
      .unwrap_or_else(|| panic!("{within} is shown")),
  ];
  while let Some(element) = stack.pop() {
    if element.id() == Some(id) {
      found += 1;
    }
    stack.extend(element.children());
  }
  found
}

#[test]
fn review_page_crumbs_lead_back_to_review_and_the_status_names_the_base() {
  let mut page = Page::new();
  page.open(("Sign webhook payloads", 1));
  let meta = page.window.text("review-status-meta");
  assert!(meta.contains("into main"), "{meta}");
  page.window.press("review-crumbs-0");
  let route = Route::Project(page.project, ProjectPage::Review);
  page
    .window
    .until("Review", |window| window.navigation().route_now() == route);
}

#[test]
fn review_page_says_nothing_is_to_review_for_a_task_whose_run_is_not_in_review() {
  let mut page = Page::of("merges");
  let task = page
    .tasks
    .iter()
    .find(|task| task.title == "Add retry to webhook sender")
    .expect("the scenario seeds a run whose merge was refused");
  let key: TaskKey = task.key.parse().expect("a task key");
  let route = Route::Project(page.project, ProjectPage::TaskReview(key.clone()));
  page.window.navigation().go(route);
  page.window.until_within(GIT_WORK, "the empty state", |window| {
    window.texts("review-no-run").is_some()
  });
  assert!(
    page
      .window
      .text("review-no-run")
      .contains(&format!("Nothing to review for {key}"))
  );
  assert_eq!(
    page.window.texts("review-crumbs").expect("the breadcrumb"),
    ["Review", "/", &key.to_string()]
  );
  assert!(page.window.texts("review-diff").is_none());
}

#[test]
fn review_page_an_approved_task_is_read_only_with_its_review_kept() {
  let mut page = Page::of("review-approved");
  page.open(("Retry budget per endpoint", 1));
  assert_eq!(page.window.text("review-status-text"), "Merged into main");
  let meta = page.window.text("review-status-meta");
  assert!(
    meta.starts_with("approved by you ") && (meta.contains("fast-forward to ") || meta.contains("merge commit ")),
    "who approved it, how it landed and when: {meta}"
  );
  assert_eq!(page.window.text("merge-attempt-title"), "Retry budget per endpoint");
  for control in [
    "review-approve",
    "review-approve-reason",
    "review-tray",
    "review-composer",
    "review-changed",
  ] {
    assert!(
      page.window.texts(control).is_none(),
      "{control} is not on an approved page"
    );
  }
  assert!(page.window.texts("review-open-task").is_some(), "Open task stays");
  let diff = page.whole_diff();
  for kept in [
    "src/webhooks/sender.rs",
    "Cap the sum of the delay and the jitter",
    "Capped the sum: the delay",
    "Addressed in",
    "Answered · no code changed",
    "The budget counts every attempt",
    "Not sent",
  ] {
    assert!(diff.contains(kept), "{kept} in {diff}");
  }
  for action in ["Reply", "Resolve", "Ask again", "Stop answering", "Edit", "Delete"] {
    assert!(!diff.contains(action), "no {action} on an approved page: {diff}");
  }
}

#[test]
fn review_page_an_approved_task_shows_no_plus_and_opens_no_comment_box() {
  let mut page = Page::of("review-approved");
  page.open(("Retry budget per endpoint", 1));
  let line = first_line(&mut page, "review-line-1-new-");
  page.window.hover(&line);
  assert_eq!(count(&mut page, "review-diff", "review-line-add"), 0);
  page.window.press(&line);
  page.window.pass();
  assert!(
    page.window.texts("review-composer").is_none(),
    "pressing a line opens no box"
  );
  assert!(page.window.texts("review-tray").is_none());
}

#[test]
fn review_page_a_merge_automation_made_is_not_called_approved_by_you() {
  let mut page = Page::of("review-approved");
  page.open(("Honor Retry-After on 429 and 503", 2));
  assert_eq!(page.window.text("review-status-text"), "Merged into main");
  let meta = page.window.text("review-status-meta");
  assert!(
    meta.starts_with("merged without a person's approval") && !meta.contains("approved by you"),
    "{meta}"
  );
}
