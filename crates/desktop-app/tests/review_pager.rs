//! The pager of a task's review page (ORC-796, `Review - v2` 02 `gSMiI` `Pager` `TIPCA`, rule 2 of `b6M3lS`) in the
//! app's real window, on the QA seed's `review-list` scenario: *N of M waiting* with ‹ › at the end of the crumbs row,
//! which go through *Waiting for you* in the list's order and are dead at its ends, `[` and `]` doing the same unless a
//! modifier is held, no pager on a task of *With the agent* or *Approved today*, and a task that leaves the group
//! being approved: the page stays on it and counts the rest. Needs `git`, as the seed does.
//! `cargo test -p orchester-app --test review_pager review_pager`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
mod support;

use std::time::Duration;

use lurq::node::element::ElementRef;
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};

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
  fn new() -> Self {
    let data = Data::new();
    let repo = data.dir.path().join("repo-review-list");
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo,
      scenario: scenarios::named("review-list").expect("a known scenario"),
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

  fn route(&self, key: &str) -> Route {
    Route::Project(self.project, ProjectPage::TaskReview(task(key)))
  }

  /// The keys of the list's rows in the order it draws them: *Waiting for you*'s first.
  fn listed(&mut self) -> Vec<String> {
    let route = Route::Project(self.project, ProjectPage::Review);
    self.window.navigation().go(route);
    self.window.until_within(GIT_WORK, "the list", |window| {
      window.tree.get_element_by_id("review-list").is_some()
    });
    let mut keys = Vec::new();
    collect(
      self.window.tree.get_element_by_id("review-list").expect("the list"),
      &mut keys,
    );
    keys
  }

  /// The key of the row, among the `listed` ones, whose words hold `title`.
  fn key_of(&mut self, listed: &[String], title: &str) -> String {
    for key in listed {
      if self.window.text(&format!("review-row-{key}")).contains(title) {
        return key.clone();
      }
    }
    panic!("a row holds {title}");
  }

  /// Opens the review of `key` and waits until the page is read, whatever it then shows.
  fn open(&mut self, key: &str) {
    let route = self.route(key);
    self.window.navigation().go(route);
    self.window.until_within(GIT_WORK, "the page", |window| {
      window.tree.get_element_by_id("screen-review").is_some()
        && window.tree.get_element_by_id("review-reading").is_none()
    });
  }

  fn pager(&mut self) -> Option<String> {
    self.window.texts("review-pager-words").map(|words| words.join(" "))
  }

  fn at(&self) -> Route {
    self.window.navigation().route_now()
  }

  /// Until the pager reads `words`.
  fn until_pager(&mut self, words: &str) {
    self.window.until_within(GIT_WORK, words, |window| {
      window
        .texts("review-pager-words")
        .is_some_and(|read| read.join(" ") == words)
    });
  }

  fn key_down(&mut self, key: &str, code: &str, (shift, ctrl): (bool, bool)) {
    self
      .window
      .tree
      .key_down(key.to_owned(), code.to_owned(), shift, ctrl, false);
    self.window.pass();
  }
}

fn task(key: &str) -> TaskKey {
  key.parse().expect("a task key")
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
fn review_pager_goes_through_waiting_for_you_in_the_lists_order_and_its_ends_are_dead() {
  let mut seeded = Seeded::new();
  let waiting: Vec<String> = seeded.listed().into_iter().take(3).collect();
  seeded.open(&waiting[1]);
  seeded.until_pager("2 of 3 waiting");
  seeded.window.press("review-pager-next");
  seeded.until_pager("3 of 3 waiting");
  assert_eq!(seeded.at(), seeded.route(&waiting[2]), "› opens the next one");
  seeded.window.press("review-pager-next");
  assert_eq!(seeded.at(), seeded.route(&waiting[2]), "the last one has no next");
  seeded.window.press("review-pager-previous");
  seeded.until_pager("2 of 3 waiting");
  seeded.window.press("review-pager-previous");
  seeded.until_pager("1 of 3 waiting");
  assert_eq!(seeded.at(), seeded.route(&waiting[0]), "‹ opens the one before");
  seeded.window.press("review-pager-previous");
  assert_eq!(seeded.at(), seeded.route(&waiting[0]), "the first one has no previous");
}

#[test]
fn review_pager_square_brackets_do_what_the_arrows_do_unless_a_modifier_is_held() {
  let mut seeded = Seeded::new();
  let waiting: Vec<String> = seeded.listed().into_iter().take(3).collect();
  seeded.open(&waiting[0]);
  seeded.until_pager("1 of 3 waiting");
  seeded.key_down("[", "BracketLeft", (false, false));
  assert_eq!(seeded.at(), seeded.route(&waiting[0]), "the first one has no previous");
  seeded.key_down("]", "BracketRight", (false, true));
  assert_eq!(seeded.at(), seeded.route(&waiting[0]), "Ctrl+] is not the pager's");
  seeded.key_down("]", "BracketRight", (false, false));
  seeded.until_pager("2 of 3 waiting");
  assert_eq!(seeded.at(), seeded.route(&waiting[1]));
  seeded.key_down("[", "BracketLeft", (false, false));
  seeded.until_pager("1 of 3 waiting");
  assert_eq!(seeded.at(), seeded.route(&waiting[0]));
}

#[test]
fn review_pager_a_task_opened_from_the_other_groups_shows_none() {
  let mut seeded = Seeded::new();
  let listed = seeded.listed();
  let others =
    ["Delivery log per endpoint", "Retry budget per endpoint"].map(|title| (title, seeded.key_of(&listed, title)));
  for (title, key) in others {
    seeded.open(&key);
    assert_eq!(seeded.pager(), None, "{title}");
    assert!(seeded.window.texts("review-pager").is_none(), "{title}");
  }
}

#[test]
fn review_pager_a_task_that_leaves_the_group_stays_and_the_pager_counts_the_rest() {
  let mut seeded = Seeded::new();
  let listed = seeded.listed();
  let health = seeded.key_of(&listed, "Endpoint health page");
  seeded.open(&health);
  seeded.window.until_within(GIT_WORK, "the new commits", |window| {
    window.tree.get_element_by_id("review-changed-take").is_some()
  });
  seeded.window.press("review-changed-take");
  seeded.window.until_within(GIT_WORK, "the commits taken in", |window| {
    window.texts("review-approve-reason").is_none()
  });
  assert!(seeded.pager().is_some_and(|words| words.ends_with("of 3 waiting")));
  seeded.window.press("review-approve");
  seeded.until_pager("2 waiting");
  assert_eq!(seeded.at(), seeded.route(&health), "the page stays on the task");
  seeded.window.press("review-pager-next");
  assert_ne!(
    seeded.at(),
    seeded.route(&health),
    "an arrow leads to one that still waits"
  );
}
