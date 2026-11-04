//! Review › Merges (ORC-362, 09 `Os7b3` of `Review · redesign`) in the app's real window, on the QA seed's merges
//! scenarios (`examples/seed`): the hub's third tab, its own route by tab, history, *Jump to* and `navigate`, with its
//! count in `warning`; *Not merged* with each outcome's line and action, *Try again* merging as the task page does,
//! Orchester's own retry followed live, and *Put files back* putting a stopped bring-in's file back; *Waiting for your
//! review* with *Changed since your review*;
//! the merged runs by local day, older days read as the list scrolls; and *No merges yet*. Needs `git`, as the seed
//! does. `cargo test -p orchester-app --test review_merges review_merges`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
mod support;

use std::{path::PathBuf, time::Duration};

use lurq::app::events::ScrollPhase;
use orchester_app::{
  mcp::tools::navigate,
  routes::{OverviewTab, ProjectPage, Route},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_exec::local_git::IntegrationLease;
use serde_json::json;

use crate::{
  kontur::{SeedRequest, scenarios},
  support::{Data, Window},
};

/// How long a merge with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);

struct Seeded {
  window: Window,
  project: ProjectId,
  repo: PathBuf,
}

impl Seeded {
  fn new(scenario: &str) -> Self {
    let data = Data::new();
    let repo = data.dir.path().join(format!("repo-{scenario}"));
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: repo.clone(),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let launch = data.launch();
    let mut window = Window::of(data, launch);
    window.tree.resize(1440, 1400);
    Self {
      window,
      project: seeded.project,
      repo,
    }
  }

  fn merges(&mut self) {
    self
      .window
      .navigation()
      .go(Route::Project(self.project, ProjectPage::Merges));
    self.until_read();
  }

  /// Until the merges are read: their sections, or *No merges yet*. The info line stands while they are read too.
  fn until_read(&mut self) {
    self.window.until_within(GIT_WORK, "the merges", |window| {
      window.texts("merges-info").is_some() && window.tree.get_element_by_id("merges-loading").is_none()
    });
  }

  fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// The ids of the rows of the section `section`, in order.
  fn rows(&mut self, section: &str) -> Vec<String> {
    let rows = format!("{section}-rows");
    self.window.tree.get_element_by_id(&rows).map_or_else(Vec::new, |rows| {
      rows
        .children()
        .into_iter()
        .filter_map(|row| row.id().map(str::to_owned))
        .collect()
    })
  }

  /// The row of `section` whose words hold `title`.
  fn row(&mut self, section: &str, title: &str) -> String {
    let rows = self.rows(section);
    rows
      .into_iter()
      .find(|row| self.window.text(row).contains(title))
      .unwrap_or_else(|| panic!("{section} lists {title}"))
  }
}

#[test]
fn review_merges_is_the_hubs_third_tab_by_tab_history_jump_to_and_navigate() {
  let mut seeded = Seeded::new("merges");
  let navigation = seeded.window.navigation();
  let project = seeded.project;
  navigation.go(Route::Project(project, ProjectPage::Overview(OverviewTab::default())));
  seeded
    .window
    .until("the sidebar", |window| window.texts("nav-review").is_some());
  seeded.window.press("nav-review");
  seeded
    .window
    .until("the Review hub", |window| window.texts("review-hub").is_some());
  let tabs = seeded.window.texts("review-hub-tabs").expect("the hub's tabs");
  let names: Vec<&str> = ["Merge queue", "Pull requests", "Merges"]
    .into_iter()
    .filter(|name| tabs.iter().any(|tab| tab == name))
    .collect();
  assert_eq!(names, ["Merge queue", "Pull requests", "Merges"], "{tabs:?}");
  // One run not merged and two waiting for review need the person.
  seeded.window.until_within(GIT_WORK, "the Merges count", |window| {
    window
      .texts("review-hub-tab-merges-count")
      .is_some_and(|texts| texts == ["3"])
  });
  seeded.window.press("review-hub-tab-merges");
  let merges = Route::Project(project, ProjectPage::Merges);
  seeded.until_read();
  assert_eq!(navigation.route_now(), merges);
  assert!(merges.path().ends_with("/merges"), "{}", merges.path());
  assert_eq!(Route::parse(&merges.path()).expect("the path reads"), merges);
  navigation.back();
  seeded.window.until("Back to the Merge queue", |window| {
    window.texts("screen-merge_queue").is_some()
  });
  navigation.forward();
  seeded
    .window
    .until("Forward to Merges", |window| window.texts("screen-merges").is_some());
  assert_eq!(navigation.route_now(), merges);
  navigation.go(Route::Project(project, ProjectPage::MergeQueue));
  seeded.window.press("sidebar-jump");
  seeded
    .window
    .until("Jump to", |window| window.texts("jump-to").is_some());
  seeded.window.set("jump-to-field", "merges");
  seeded.window.until("the Merges entry", |window| {
    window.texts("jump-to-pages-merges").is_some()
  });
  seeded.window.press("jump-to-pages-merges");
  assert_eq!(navigation.route_now(), merges);
  navigation.go(Route::Project(project, ProjectPage::MergeQueue));
  let answer = navigate(
    &navigation,
    &json!({ "route": "merges", "project": project.to_string() }),
  )
  .expect("navigate opens Merges");
  assert_eq!(answer["route"]["path"], json!(merges.path()));
  assert_eq!(navigation.route_now(), merges);
}

#[test]
fn review_merges_lists_not_merged_waiting_for_review_and_merged_runs_by_day() {
  let mut seeded = Seeded::new("merges");
  seeded.merges();
  let lead = seeded.window.text("merges-info");
  assert!(lead.contains("Every merge into main"), "{lead}");
  assert!(
    lead.contains("Base main") && lead.contains("Checkout on main · clean"),
    "{lead}"
  );
  let held = seeded.row("merges-not-merged", "Add retry to webhook sender");
  let text = seeded.window.text(&held);
  assert!(
    text.contains(
      "Refused before Git at Integration lock: another merge or local integration holds this checkout. Nothing changed."
    ),
    "{text}"
  );
  assert!(
    text.contains("You · ") && text.contains("2 attempts") && text.contains("Try again"),
    "{text}"
  );
  assert_eq!(seeded.rows("merges-waiting").len(), 2);
  let queued = seeded.row("merges-waiting", "Sign webhook payloads");
  let text = seeded.window.text(&queued);
  assert!(
    text.contains("Every step passed") && text.contains("merges into main once you approve"),
    "{text}"
  );
  assert!(
    text.contains("Merge queue · since") && text.contains("Review changes"),
    "{text}"
  );
  let moved = seeded.row("merges-waiting", "Endpoint health page");
  let text = seeded.window.text(&moved);
  assert!(
    text.contains("Changed since your review: 2 commits after ")
      && text.contains(" · nothing merged · review again")
      && text.contains("Review again"),
    "{text}"
  );
  let headings: Vec<String> = (0..3)
    .map(|day| seeded.window.text(&format!("merges-day-{day}-head")))
    .collect();
  assert!(
    headings[0].starts_with("Today") && headings[0].ends_with('3'),
    "{headings:?}"
  );
  assert!(headings[1].starts_with("Yesterday"), "{headings:?}");
  assert!(
    !headings[2].starts_with("Today") && !headings[2].starts_with("Yesterday"),
    "{headings:?}"
  );
  let today = seeded.rows("merges-day-0");
  let titles: Vec<String> = today.iter().map(|row| seeded.window.text(row)).collect();
  assert!(
    titles[0].contains("Retry budget per endpoint"),
    "newest first: {titles:?}"
  );
  assert!(titles[0].contains("Merge queue · "), "{titles:?}");
  assert!(
    titles[0].contains("· worktree removed · branch deleted"),
    "a run whose branch went says so: {titles:?}"
  );
  assert!(titles[2].contains("after 1 refused attempt"), "{titles:?}");
  assert!(
    !titles[2].contains("branch deleted"),
    "a run whose branch stays does not: {titles:?}"
  );
  assert!(
    !seeded.shows(&format!("{}-action", today[0])),
    "a merged run has no action"
  );
}

#[test]
fn review_merges_tells_every_outcome_with_the_action_that_moves_it_on() {
  let mut seeded = Seeded::new("merges-outcomes");
  seeded.merges();
  let cases = [
    (
      "Add retry to webhook sender",
      "Refused before Git at Integration lock",
      "Try again",
    ),
    (
      "Per-endpoint rate limits",
      "Stopped in the merge queue at Project database: orchester.db-wal is used by another program",
      "Try again",
    ),
    ("Per-endpoint secrets", "Conflicts with main in 2 files.", "Resolve"),
    (
      "Delivery dashboard",
      "not recorded: the project database stayed busy",
      "Record merge",
    ),
    (
      "Rate limit per endpoint",
      "Git stopped answering at Merge · stopped after 2 min · checkout and main untouched",
      "Try again",
    ),
    (
      "Payload size guard",
      "Stopped while bringing the merge in · 1 file left changed · main did not move",
      "Put files back",
    ),
  ];
  for (title, line, action) in cases {
    let row = seeded.row("merges-not-merged", title);
    let text = seeded.window.text(&row);
    assert!(text.contains(line), "{title}: {text}");
    assert_eq!(seeded.window.text(&format!("{row}-action")), action, "{title}");
  }
  let waiting = seeded.row("merges-waiting", "Sign webhook payloads");
  let text = seeded.window.text(&waiting);
  assert!(text.contains("2 comments not sent"), "{text}");
  // Put files back puts the run's file back: the run can be tried again.
  let left = seeded.row("merges-not-merged", "Payload size guard");
  seeded.window.press(&format!("{left}-action"));
  let action = format!("{left}-action");
  seeded
    .window
    .until_within(GIT_WORK, "the file to be put back", |window| {
      window.texts(&action).is_some_and(|texts| texts == ["Try again"])
    });
  assert!(!seeded.shows("merges-not-done"), "the put-back went through");
}

#[test]
fn review_merges_try_again_merges_as_the_task_page_does() {
  let mut seeded = Seeded::new("merges");
  seeded.merges();
  let held = seeded.row("merges-not-merged", "Add retry to webhook sender");
  seeded.window.press(&format!("{held}-action"));
  seeded.window.until_within(GIT_WORK, "the merge to land", |window| {
    window.texts("merges-not-merged").is_none()
      && window
        .texts("merges-day-0-rows")
        .is_some_and(|texts| texts.iter().any(|text| text == "Add retry to webhook sender"))
  });
  let today = seeded.row("merges-day-0", "Add retry to webhook sender");
  let text = seeded.window.text(&today);
  assert!(
    text.contains("You · ") && text.contains("after 2 refused attempts"),
    "{text}"
  );
  seeded.window.until("the count to drop", |window| {
    window
      .texts("review-hub-tab-merges-count")
      .is_some_and(|texts| texts == ["2"])
  });
}

/// While Orchester waits to try a merge again, no job state changes: the list watches the run that merges once a
/// second, the window's timers firing meanwhile, so its row turns to *Retrying by itself* with *Try now* as the wait
/// begins, and counts the next attempt as each try is refused again; once the lease is let go, *Try now* tries at once
/// and the row tells how that try ended.
#[test]
fn review_merges_follows_orchesters_retry_live() {
  let mut seeded = Seeded::new("merges-outcomes");
  seeded.merges();
  let row = seeded.row("merges-not-merged", "Add retry to webhook sender");
  let action = format!("{row}-action");
  let held = IntegrationLease::acquire(&seeded.repo).expect("another operation's lease");
  seeded.window.press(&action);
  let retrying = |attempt: &'static str| {
    let (row, action) = (row.clone(), action.clone());
    move |window: &mut Window| {
      window.tree.tick_timers();
      let line = format!("Retrying by itself · attempt {attempt} of 3 in");
      window
        .texts(&row)
        .is_some_and(|texts| texts.iter().any(|text| text.contains(&line)))
        && window.texts(&action).is_some_and(|texts| texts == ["Try now"])
    }
  };
  seeded.window.until_within(GIT_WORK, "the row to retry", retrying("1"));
  seeded.window.press(&action);
  seeded.window.until_within(GIT_WORK, "the next attempt", retrying("2"));
  drop(held);
  seeded.window.press(&action);
  seeded.window.until_within(GIT_WORK, "the try to end", |window| {
    window.tree.tick_timers();
    window.texts(&action).is_some_and(|texts| texts != ["Try now"])
  });
  let text = seeded.window.text(&row);
  assert!(!text.contains("Retrying by itself"), "{text}");
}

#[test]
fn review_merges_reads_older_days_as_the_list_scrolls() {
  let mut seeded = Seeded::new("merges-many");
  seeded.merges();
  let listed =
    |seeded: &mut Seeded| -> usize { (0..20).map(|day| seeded.rows(&format!("merges-day-{day}")).len()).sum() };
  assert_eq!(listed(&mut seeded), 30, "a page at first");
  for _ in 0..40 {
    if listed(&mut seeded) == 34 {
      break;
    }
    wheel(&mut seeded.window, -600.0);
    seeded.window.tree.tick_futures();
    seeded.window.pass();
  }
  seeded.window.until_within(GIT_WORK, "the older days", |window| {
    window.texts("merges-reading-more").is_none()
  });
  assert_eq!(listed(&mut seeded), 34, "the rest once the list scrolled near its end");
  assert!(!seeded.shows("merges-not-merged") && !seeded.shows("merges-waiting"));
}

#[test]
fn review_merges_without_any_attempt_says_no_merges_yet() {
  let mut seeded = Seeded::new("merges-empty");
  seeded.merges();
  seeded
    .window
    .until("No merges yet", |window| window.texts("merges-empty").is_some());
  let empty = seeded.window.text("merges-empty");
  assert!(
    empty.contains("No merges yet") && empty.contains("Every merge into main shows here once it is tried"),
    "{empty}"
  );
  seeded.window.until("the tab without a count", |window| {
    window.texts("review-hub-tab-merges").is_some() && window.texts("review-hub-tab-merges-count").is_none()
  });
}

/// Turns the wheel over the list: below 0 scrolls toward its end.
fn wheel(window: &mut Window, delta: f32) {
  let bounds = window
    .tree
    .get_element_by_id_mut("merges-main")
    .and_then(|element| element.bounds())
    .expect("the list is laid out");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.scroll(x, y, 0.0, delta, ScrollPhase::Scroll);
  window.pass();
}
