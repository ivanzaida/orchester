//! Review's list (ORC-794, `Review - v2` `j1Drb` 01 `i6S42`, 03 `QpQTI`, 04 `M1LAx`) in the app's real window, on the
//! QA seed's `review-list` scenario: the groups with their counts, each row's state line, files and lines, a row and a
//! hovered row's Review button opening the task's review, the groups folding (*Approved today* folded once read), the
//! search, the sidebar's count, and what reading never moves: *Waiting for you*'s bones equal the count, nothing waits
//! is drawn at once, and the lower groups come under what is there. Needs `git`, as the seed does.
//! `cargo test -p orchester-app --test review_list review_list`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
mod support;

use std::time::Duration;

use lurq::node::element::ElementRef;
use orchester_app::{
  project::TasksState,
  routes::{ProjectPage, Route, TaskList},
};
use orchester_core::{id::ProjectId, time::Timestamp};

use crate::{
  kontur::{SeedRequest, scenarios},
  support::{Data, Window},
};

/// How long a read with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);

struct Seeded {
  window: Window,
  project: ProjectId,
}

impl Seeded {
  fn new(scenario: &str) -> Self {
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
    let launch = data.launch();
    let mut window = Window::of(data, launch);
    window.tree.resize(1440, 900);
    Self {
      window,
      project: seeded.project,
    }
  }

  fn route(&self) -> Route {
    Route::Project(self.project, ProjectPage::Review)
  }

  /// Opens the project's Tasks and waits until its tasks are read, so only the Review list's own read is left to wait
  /// for.
  fn tasks_read(&mut self) {
    self
      .window
      .navigation()
      .go(Route::Project(self.project, ProjectPage::Tasks(TaskList::default())));
    let feed = self.window.projects().feed(self.project);
    self.window.until_within(GIT_WORK, "the tasks", |_| {
      matches!(feed.tasks.get_untracked(), TasksState::Ready(_))
    });
  }

  fn review(&mut self) {
    let route = self.route();
    self.window.navigation().go(route);
  }

  /// Until the list is read: its lower groups and rows, or a group of 0.
  fn until_read(&mut self) {
    self.window.until_within(GIT_WORK, "the list", |window| {
      window.tree.get_element_by_id("review-list").is_some()
    });
  }

  fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// The keys of the rows that are drawn, in order.
  fn keys(&mut self) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(list) = self.window.tree.get_element_by_id("review-list") {
      collect(list, &mut keys);
    }
    keys
  }

  /// The key of the row whose words hold `title`.
  fn key_of(&mut self, title: &str) -> String {
    for key in self.keys() {
      if self.window.text(&format!("review-row-{key}")).contains(title) {
        return key;
      }
    }
    panic!("a row holds {title}");
  }

  fn row(&mut self, title: &str) -> String {
    let key = self.key_of(title);
    self.window.text(&format!("review-row-{key}"))
  }
}

fn collect(element: ElementRef<'_>, keys: &mut Vec<String>) {
  if let Some(key) = element.id().and_then(|id| id.strip_prefix("review-row-")) {
    keys.push(key.to_owned());
  }
  for child in element.children() {
    collect(child, keys);
  }
}

#[test]
fn review_list_groups_and_rows_say_where_each_task_stands() {
  let mut seeded = Seeded::new("review-list");
  seeded.review();
  seeded.until_read();
  assert_eq!(seeded.window.text("review-title"), "Review");
  for (group, count) in [("waiting", "3"), ("with-agent", "1"), ("approved", "2")] {
    assert_eq!(
      seeded.window.text(&format!("review-group-{group}-count")),
      count,
      "{group}"
    );
  }
  assert_eq!(seeded.keys().len(), 6);
  assert!(seeded.row("Sign webhook payloads").contains("1 comment not sent"));
  assert!(seeded.row("Endpoint health page").contains("Changed since your review"));
  assert!(seeded.row("Rotate API keys from settings").contains("Conflicts with"));
  assert!(
    seeded
      .row("Delivery log per endpoint")
      .contains("Implement is on 1 comment")
  );
  let merged = seeded.row("Retry budget per endpoint");
  assert!(merged.contains("Merged into"), "{merged}");
  assert!(
    merged.contains("file") && merged.contains('+') && merged.contains('\u{2212}'),
    "{merged}"
  );
  assert_eq!(
    seeded.window.text("nav-review"),
    "Review 3",
    "the sidebar counts what waits for you"
  );
}

#[test]
fn review_list_a_row_and_a_hovered_rows_review_button_open_the_tasks_review() {
  let mut seeded = Seeded::new("review-list");
  seeded.review();
  seeded.until_read();
  let key = seeded.key_of("Sign webhook payloads");
  assert!(
    !seeded.shows(&format!("review-open-{key}")),
    "the button shows under the pointer only"
  );
  seeded.window.hover(&format!("review-row-{key}"));
  assert!(seeded.shows(&format!("review-open-{key}")));
  assert!(
    !seeded.shows(&format!("review-counts-{key}")),
    "it takes the counts' place"
  );
  seeded.window.press(&format!("review-open-{key}"));
  let task = key.parse().expect("a task key");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::TaskReview(task))
  );
  seeded.window.navigation().back();
  seeded.until_read();
  let key = seeded.key_of("Endpoint health page");
  seeded.window.press(&format!("review-row-{key}"));
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(
      seeded.project,
      ProjectPage::TaskReview(key.parse().expect("a task key"))
    )
  );
}

#[test]
fn review_list_groups_fold_and_approved_today_is_folded_once_read() {
  let mut seeded = Seeded::new("review-list");
  seeded.review();
  seeded.until_read();
  assert_eq!(seeded.keys().len(), 6, "every group is open on a first opening");
  seeded.window.press("review-group-waiting");
  assert_eq!(seeded.keys().len(), 3, "Waiting for you's rows are folded away");
  assert_eq!(
    seeded.window.text("review-group-waiting-count"),
    "3",
    "its header keeps its count"
  );
  seeded.window.press("review-group-waiting");
  assert_eq!(seeded.keys().len(), 6);
  // Opened again, the list last read shows at once and *Approved today* is folded: it was read.
  seeded.tasks_read();
  seeded.review();
  seeded.until_read();
  assert_eq!(
    seeded.keys().len(),
    4,
    "Approved today keeps its header and folds its rows"
  );
  assert_eq!(seeded.window.text("review-group-approved-count"), "2");
  seeded.window.press("review-group-approved");
  assert_eq!(seeded.keys().len(), 6);
}

#[test]
fn review_list_search_narrows_the_rows_by_key_and_title() {
  let mut seeded = Seeded::new("review-list");
  seeded.review();
  seeded.until_read();
  let key = seeded.key_of("Rotate API keys");
  seeded.window.set("review-search", "rotate api");
  assert_eq!(seeded.keys(), vec![key.clone()]);
  assert!(
    !seeded.shows("review-group-approved"),
    "a group without a match is left out"
  );
  seeded.window.set("review-search", &key.to_lowercase());
  assert_eq!(seeded.keys(), vec![key]);
  seeded.window.set("review-search", "nothing like this");
  assert!(seeded.keys().is_empty());
  assert!(seeded.shows("review-no-match"), "something waits, the search hides it");
}

#[test]
fn review_list_reading_gives_waiting_for_you_as_many_bones_as_the_count_and_nothing_above_moves() {
  let mut seeded = Seeded::new("review-list");
  seeded.tasks_read();
  let hold = seeded
    .window
    .launch
    .services()
    .expect("the session started")
    .hold
    .clone();
  hold.set(true);
  seeded.review();
  seeded.window.until("the bones", |window| {
    window.tree.get_element_by_id("review-loading-rows").is_some()
  });
  let bones = seeded
    .window
    .tree
    .get_element_by_id("review-loading-rows")
    .expect("the bones")
    .children()
    .len();
  assert_eq!(
    bones,
    1 + 3,
    "Waiting for you's header and one row of bones for each waiting task"
  );
  assert_eq!(seeded.window.text("review-title"), "Review", "the head shows at once");
  assert!(!seeded.shows("review-group-with-agent") && !seeded.shows("review-group-approved"));
  let before = top_of(&mut seeded.window, "review-loading-rows");
  hold.set(false);
  seeded.until_read();
  assert_eq!(
    top_of(&mut seeded.window, "review-list"),
    before,
    "the rows stand where their bones stood"
  );
  assert_eq!(seeded.keys().len(), 6);
  assert!(seeded.shows("review-group-with-agent") && seeded.shows("review-group-approved"));
}

#[test]
fn review_list_with_nothing_waiting_is_a_group_of_0_at_once_never_after_bones() {
  let mut seeded = Seeded::new("merges-empty");
  seeded.tasks_read();
  let hold = seeded
    .window
    .launch
    .services()
    .expect("the session started")
    .hold
    .clone();
  hold.set(true);
  seeded.review();
  seeded.window.until("the page", |window| {
    window.tree.get_element_by_id("review-empty").is_some()
  });
  assert!(!seeded.shows("review-loading"), "no bones");
  assert_eq!(seeded.window.text("review-group-waiting-count"), "0");
  assert_eq!(
    seeded.window.text("review-empty"),
    "Nothing waits for your review a task shows up here when its run finishes every step"
  );
  assert!(!seeded.shows("review-group-with-agent") && !seeded.shows("review-group-approved"));
  hold.set(false);
  seeded.window.until_within(GIT_WORK, "the read", |window| {
    window.tree.get_element_by_id("review-empty").is_some()
  });
  assert!(!seeded.shows("review-loading"));
  assert_eq!(seeded.window.text("nav-review"), "Review", "no count for none");
}

/// Where the element `id` starts from the window's top.
fn top_of(window: &mut Window, id: &str) -> f32 {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .map(|bounds| bounds.y)
    .unwrap_or_else(|| panic!("{id} is laid out"))
}
