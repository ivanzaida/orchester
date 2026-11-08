//! A run's merge attempt page (ORC-363, `Review · redesign` 09b to 09l) in the app's real window, on the QA seed's
//! `merge-attempts` scenario (`examples/seed`): its route under its task from history and `navigate`, with its
//! breadcrumb, header actions and status; *Checks* in order, the passed ones folded, the refused one with its cause,
//! the later ones not reached; *Attempts* expanding into their own checks, Orchester's retries among them; and the
//! actions that move a run on: *Record merge*, *Put files back*, and *Copy details*; *Try again* back into the merge
//! queue and Orchester's retry are in `retries`. Needs `git`, as the seed does.
//! `cargo test -p orchester-app --test merge_attempt_page merge_attempt_page`.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod layout;
mod merged;
mod resolved;
mod retries;
#[path = "../support/mod.rs"]
mod support;

use std::{path::PathBuf, time::Duration};

use orchester_app::{
  mcp::tools::navigate,
  routes::{ProjectPage, Route, TaskView},
};
use orchester_core::{
  id::ProjectId,
  store::{MergeStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::json;

use crate::{
  kontur::{SeedRequest, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long a merge with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);

struct Page {
  window: Window,
  stand_ins: StandIns,
  project: ProjectId,
  repo: PathBuf,
}

impl Page {
  fn new() -> Self {
    let data = Data::new();
    let repo = data.dir.path().join("repo-merge-attempts");
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: repo.clone(),
      scenario: scenarios::named("merge-attempts").expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let launch = data.launch();
    let (mut window, stand_ins) = Window::launched_with_stand_ins(data, launch);
    window.tree.resize(1440, 1400);
    Self {
      window,
      stand_ins,
      project: seeded.project,
      repo,
    }
  }

  /// Opens the attempt page of the task titled `title`'s run, at its route under the task; its route.
  fn open(&mut self, title: &str) -> Route {
    let db = ProjectDb::open(&self.repo).expect("the project database");
    let task = db
      .tasks()
      .expect("the tasks")
      .into_iter()
      .find(|task| task.details().title.as_str() == title)
      .unwrap_or_else(|| panic!("the scenario seeds {title}"));
    let run = db
      .runs_of_task(task.id())
      .expect("the runs")
      .into_iter()
      .find_map(Result::ok)
      .expect("its run");
    self.window.navigation().go(Route::Project(
      self.project,
      ProjectPage::TaskMerge(task.key().clone(), run.number()),
    ));
    self.until_read();
    self.window.navigation().route_now()
  }

  /// Until the page is read: its status line.
  fn until_read(&mut self) {
    self.window.until_within(GIT_WORK, "the attempt page", |window| {
      window.texts("merge-attempt-status-text").is_some()
    });
  }

  fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// Until the status line's words start with `status`, the window's timers firing meanwhile: the page looks at
  /// Orchester's wait to try again once a second, and its countdown ticks.
  fn until_status(&mut self, status: &str) {
    self.window.until_within(GIT_WORK, status, |window| {
      window.tree.tick_timers();
      window
        .texts("merge-attempt-status-text")
        .is_some_and(|texts| texts.iter().any(|text| text.starts_with(status)))
    });
  }
}

#[test]
fn merge_attempt_page_opens_from_its_row_route_history_and_navigate() {
  let mut page = Page::new();
  let route = page.open("Add retry to webhook sender");
  let Route::Project(project, ProjectPage::TaskMerge(key, number)) = route.clone() else {
    panic!("a row opens the run's attempts: {route:?}");
  };
  assert!(
    route.path().ends_with(&format!("/tasks/{key}/merge/{}", number.get())),
    "{}",
    route.path()
  );
  assert_eq!(Route::parse(&route.path()).expect("the path reads"), route);
  let crumbs = page.window.texts("merge-attempt-crumbs").expect("the breadcrumb");
  assert_eq!(
    crumbs,
    [
      "Tasks",
      "/",
      &key.to_string(),
      "/",
      &format!("Merge · run {}", number.get())
    ],
    "{crumbs:?}"
  );
  assert_eq!(page.window.text("merge-attempt-title"), "Add retry to webhook sender");
  assert_eq!(page.window.text("merge-attempt-copy-details"), "Copy details");
  assert_eq!(page.window.text("merge-attempt-try-again"), "Try again");
  assert_eq!(page.window.text("merge-attempt-status-text"), "Not merged into main");
  let meta = page.window.text("merge-attempt-status-meta");
  assert!(
    meta.contains(&format!("run {}", number.get()))
      && meta.contains("2 attempts")
      && meta.contains("by you from the task page"),
    "{meta}"
  );
  let explanation = page.window.text("merge-attempt-explanation");
  assert!(
    explanation.starts_with("Another merge or local integration held this project's checkout"),
    "{explanation}"
  );
  let navigation = page.window.navigation();
  navigation.go(Route::Project(
    project,
    ProjectPage::Task(key.clone(), TaskView::default()),
  ));
  page
    .window
    .until("the task's page", |window| window.texts("task-title").is_some());
  navigation.back();
  page.until_read();
  assert_eq!(navigation.route_now(), route);
  let answer = navigate(
    &navigation,
    &json!({ "route": "task", "project": project.to_string(), "key": key.to_string(), "run": number.get().to_string() }),
  )
  .expect("navigate opens the run's attempts");
  assert_eq!(answer["route"]["path"], json!(route.path()));
  assert_eq!(answer["route"]["params"]["key"], json!(key.to_string()));
  assert_eq!(answer["route"]["params"]["run"], json!(number.get().to_string()));
  assert_eq!(navigation.route_now(), route);
  assert!(
    navigate(
      &navigation,
      &json!({ "route": "task", "project": project.to_string(), "key": key.to_string(), "run": "8", "tab": "changes" }),
    )
    .is_err(),
    "a run's merge attempts have no tab"
  );
}

#[test]
fn merge_attempt_page_checks_list_the_stages_with_the_refused_cause_and_fold_the_passed() {
  let mut page = Page::new();
  page.open("Add retry to webhook sender");
  let names: Vec<String> = (1..=8)
    .map(|stage| page.window.text(&format!("merge-attempt-checks-{stage}-name")))
    .collect();
  assert_eq!(
    names,
    [
      "Project database",
      "Integration lock",
      "Scratch folder",
      "Checkout identity",
      "Git metadata",
      "Result already in Git",
      "Merge",
      "Record"
    ],
    "the checks in the order they run: the project database is held before the integration lock is taken"
  );
  assert_eq!(page.window.text("merge-attempt-checks-1-result"), "Passed");
  assert_eq!(
    page.window.text("merge-attempt-checks-2-result"),
    "Held by another operation"
  );
  assert_eq!(
    page.window.text("merge-attempt-checks-2-cause-text"),
    "held by another operation"
  );
  for stage in 3..=8 {
    assert_eq!(
      page.window.text(&format!("merge-attempt-checks-{stage}-result")),
      "Not reached"
    );
  }
  assert!(!page.shows("merge-attempt-checks-passed"), "one check passed before it");

  page.open("Rate limit per endpoint");
  let passed = page.window.text("merge-attempt-checks-passed");
  assert!(
    passed.contains("01–06") && passed.contains("6 checks passed") && passed.contains("Show"),
    "{passed}"
  );
  let parts = page
    .window
    .tree
    .get_element_by_id("merge-attempt-checks-passed")
    .and_then(|row| row.children().into_iter().next().map(|line| line.children().len()));
  assert_eq!(
    parts,
    Some(5),
    "numbers, glyph, count, names and Show: no chevron beside Show"
  );
  assert!(!page.shows("merge-attempt-checks-1"), "the passed checks are folded");
  assert_eq!(page.window.text("merge-attempt-checks-7-result"), "No answer for 2 min");
  let cause = page.window.text("merge-attempt-checks-7-cause-text");
  assert!(cause.contains("wrote nothing for 120 s"), "{cause}");
  assert_eq!(page.window.text("merge-attempt-checks-8-result"), "Not reached");
  page.window.press("merge-attempt-checks-passed");
  page.window.until("the passed checks", |window| {
    window.texts("merge-attempt-checks-1").is_some()
  });
  assert_eq!(page.window.text("merge-attempt-checks-1-result"), "Passed");
  assert!(page.window.text("merge-attempt-checks-passed").contains("Hide"));
}

#[test]
fn merge_attempt_page_refused_stage_names_its_cause_category_and_the_row_its_os_error() {
  let mut page = Page::new();
  page.open("Per-endpoint rate limits");
  assert_eq!(page.window.text("merge-attempt-checks-1-name"), "Project database");
  assert_eq!(
    page.window.text("merge-attempt-checks-1-result"),
    "Used by another program"
  );
  let cause = page.window.text("merge-attempt-checks-1-cause-text");
  assert!(cause.contains("being used by another process"), "{cause}");
  let below: Vec<String> = (2..=8)
    .map(|stage| page.window.text(&format!("merge-attempt-checks-{stage}-result")))
    .collect();
  assert!(
    below.iter().all(|result| result == "Not reached"),
    "every check after the refused one, and none above it, is not reached: {below:?}"
  );
  assert_eq!(page.window.text("merge-attempt-checks-2-name"), "Integration lock");
  assert_eq!(
    page.window.text("merge-attempt-1-outcome"),
    "Refused at Project database · os error 32"
  );
}

#[test]
fn merge_attempt_page_git_that_stopped_answering_is_told_once_with_its_time() {
  let mut page = Page::new();
  page.open("Rate limit per endpoint");
  let explanation = page.window.text("merge-attempt-explanation");
  assert!(
    explanation.starts_with("Git wrote nothing for 2 minutes at Merge"),
    "{explanation}"
  );
  assert_eq!(
    explanation.matches("wrote nothing").count(),
    1,
    "what stopped it is said once: {explanation}"
  );
  assert!(!explanation.contains("Git stopped answering"), "{explanation}");
}

#[test]
fn merge_attempt_page_facts_are_one_row_with_the_whole_folder() {
  let mut page = Page::new();
  page.open("Add retry to webhook sender");
  let folder = page.window.text("merge-attempt-fact-folder-value");
  assert_eq!(folder, page.repo.display().to_string(), "the whole folder");
  assert!(!page.window.text("merge-attempt-fact-branch-value").is_empty());
  let top = |page: &mut Page, id: &str| {
    page
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
      .y
  };
  let (branch, folder, logs) = (
    top(&mut page, "merge-attempt-fact-branch"),
    top(&mut page, "merge-attempt-fact-folder"),
    top(&mut page, "merge-attempt-log-folder"),
  );
  assert!(
    (branch - folder).abs() < 1.0 && (branch - logs).abs() < 8.0,
    "the facts and the log link stand on one line: {branch}, {folder}, {logs}"
  );
}

#[test]
fn merge_attempt_page_attempts_newest_first_expand_into_their_checks() {
  let mut page = Page::new();
  page.open("Delivery log per endpoint");
  assert_eq!(page.window.text("merge-attempt-status-text"), "Merged into main");
  assert!(page.window.text("merge-attempt-what-title").contains("What happened"));
  assert!(
    !page.shows("merge-attempt-try-again"),
    "a merged run has no primary action"
  );
  assert_eq!(page.window.text("merge-attempt-attempts-head-count"), "3");
  assert!(
    page.window.text("merge-attempt-checks-passed").contains("01–06"),
    "the checks before Git fold even when every stage passed"
  );
  assert!(!page.shows("merge-attempt-checks-1"));
  assert!(page.shows("merge-attempt-checks-7") && page.shows("merge-attempt-checks-8"));
  assert!(page.window.text("merge-attempt-3-outcome").starts_with("Merged as "));
  assert!(
    page
      .window
      .text("merge-attempt-2-outcome")
      .starts_with("Refused at Integration lock")
  );
  assert_eq!(page.window.text("merge-attempt-1-who"), "Automation");
  assert_eq!(page.window.text("merge-attempt-1-via"), "merge_run");
  page.window.press("merge-attempt-2");
  page.window.until("the attempt's checks", |window| {
    window.texts("merge-attempt-2-checks").is_some()
  });
  assert_eq!(
    page.window.text("merge-attempt-2-checks-2-result"),
    "Held by another operation"
  );
  page.window.press("merge-attempt-2");
  page.window.until("the attempt folded", |window| {
    window.texts("merge-attempt-2-checks").is_none()
  });

  page.open("Retry budget per endpoint");
  assert_eq!(page.window.text("merge-attempt-2-who"), "Orchester");
  assert_eq!(page.window.text("merge-attempt-2-via"), "retry 1 of 3");
  let time = page.window.text("merge-attempt-2-time");
  assert_eq!(
    time.matches(':').count(),
    2,
    "attempts a minute holds two of tell their seconds: {time}"
  );
}

#[test]
fn merge_attempt_page_record_merge_records_the_merge_git_made() {
  let mut page = Page::new();
  let route = page.open("Delivery dashboard");
  assert_eq!(
    page.window.text("merge-attempt-status-text"),
    "Merged by Git, not recorded"
  );
  assert_eq!(page.window.text("merge-attempt-checks-8-result"), "Not recorded");
  assert!(
    page.window.text("merge-attempt-checks-passed").contains("01–06"),
    "only the checks before Git fold"
  );
  assert!(
    page.shows("merge-attempt-checks-7"),
    "Merge keeps its own row, with the commit Git made"
  );
  page.window.press("merge-attempt-record-merge");
  page.until_status("Merged into main");
  assert!(!page.shows("merge-attempt-not-done"), "the merge was recorded");
  let Route::Project(_, ProjectPage::TaskMerge(key, number)) = route else {
    panic!("the attempt page");
  };
  let db = ProjectDb::open(&page.repo).expect("the project database");
  let task = db
    .tasks()
    .expect("the tasks")
    .into_iter()
    .find(|task| *task.key() == key)
    .expect("the task");
  let run = db
    .runs_of_task(task.id())
    .expect("the runs")
    .into_iter()
    .filter_map(Result::ok)
    .find(|run| run.number() == number)
    .expect("the run");
  let merge = db.run_merge(run.id()).expect("read").expect("recorded");
  assert_eq!(merge.into.as_str(), "main");
}

#[test]
fn merge_attempt_page_put_files_back_then_try_again() {
  let mut page = Page::new();
  page.open("Payload size guard");
  assert_eq!(
    page.window.text("merge-attempt-status-text"),
    "Stopped while bringing the merge in"
  );
  assert!(page.window.text("merge-attempt-left").contains("README.md"));
  assert!(
    page
      .window
      .text("merge-attempt-bar-body")
      .contains("Try again waits until these files match main again."),
  );
  assert_eq!(page.window.text("merge-attempt-open-folder"), "Open folder");
  assert_eq!(
    page.window.text("merge-attempt-try-again"),
    "Try again",
    "locked while the files differ"
  );
  page.window.press("merge-attempt-put-back");
  page
    .window
    .until_within(GIT_WORK, "the files to be put back", |window| {
      window.texts("merge-attempt-left").is_none()
    });
  assert!(!page.shows("merge-attempt-left"), "nothing is left changed");
  assert!(!page.shows("merge-attempt-not-done"), "the put-back went through");
  let explanation = page.window.text("merge-attempt-explanation");
  assert!(
    explanation.contains("are put back") && !explanation.contains("still holds"),
    "What stopped it no longer asks to put a file back: {explanation}"
  );
}

#[test]
fn merge_attempt_page_copy_details_copies_the_stage_cause_branch_and_revisions() {
  let mut page = Page::new();
  page.open("Add retry to webhook sender");
  page.window.press("merge-attempt-copy-details");
  page.window.until("Copied", |window| {
    window
      .texts("merge-attempt-copy-details")
      .is_some_and(|texts| texts.iter().any(|text| text == "Copied"))
  });
  let copied = page.stand_ins.copied.lock().clone();
  let [details] = copied.as_slice() else {
    panic!("one copy: {copied:?}");
  };
  for line in [
    "Add retry to webhook sender",
    "Status: Not merged into main",
    "Stage: Integration lock",
    "Cause: held by another operation",
    "Attempt: 2 at ",
    "Branch: ",
    "Into: main @ ",
    "Reviewed at: ",
  ] {
    assert!(details.contains(line), "{line}: {details}");
  }
}

#[test]
fn merge_attempt_page_conflicts_list_the_files_and_both_ways_to_resolve() {
  let mut page = Page::new();
  page.open("Per-endpoint secrets");
  assert_eq!(page.window.text("merge-attempt-status-text"), "Conflicts with main");
  assert_eq!(page.window.text("merge-attempt-conflicting-head-count"), "2");
  assert!(
    page
      .window
      .text("merge-attempt-conflict-0")
      .contains("src/webhooks/retry.rs")
  );
  assert!(page.shows("merge-attempt-resolve-in-editor"));
  assert!(page.shows("merge-attempt-ask-agent"));
  assert_eq!(
    page.window.text("merge-attempt-checks-7-result"),
    "Conflicts in 2 files"
  );
  assert!(
    !page.shows("merge-attempt-checks-7-cause"),
    "conflicts have no cause line"
  );
}

#[test]
fn merge_attempt_page_names_the_projects_runner_log_folder() {
  let mut page = Page::new();
  page.open("Add retry to webhook sender");
  assert_eq!(page.window.text("merge-attempt-log-folder"), "Runner log folder");
  assert_eq!(
    orchester_ipc::runner_log_dir(&page.repo),
    page.repo.join(".orchester").join("logs")
  );
}
