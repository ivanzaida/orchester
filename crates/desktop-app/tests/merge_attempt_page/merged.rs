//! A run merged after the attempt its page opens on (ORC-917): the page says it was merged, first, and shows the
//! attempt as history with no action that would fix it; the side column's *Into* names the branch alone, not today's
//! tip; the header's long title is cut before its actions.

use std::process::Command;

use orchester_core::{
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::BranchName,
  store::{MergeStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::Page;

/// The fix actions of an attempt that stopped at conflicts, and the other ways to move a run on.
const FIX_ACTIONS: [&str; 6] = [
  "merge-attempt-resolve-in-editor",
  "merge-attempt-ask-agent",
  "merge-attempt-try-again",
  "merge-attempt-record-merge",
  "merge-attempt-put-back",
  "merge-attempt-try-now",
];

/// The run of the task titled `title` in `page`'s project.
fn run_of(page: &Page, title: &str) -> orchester_core::run::Run {
  let db = ProjectDb::open(&page.repo).expect("the project database");
  let task = db
    .tasks()
    .expect("the tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == title)
    .expect("the scenario seeds the task");
  db.runs_of_task(task.id())
    .expect("the runs")
    .into_iter()
    .find_map(Result::ok)
    .expect("its run")
}

fn assert_history_without_fixes(page: &mut Page) {
  for id in FIX_ACTIONS {
    assert!(!page.shows(id), "{id}: a merged run has nothing to fix");
  }
  assert!(page.shows("merge-attempt-history"), "the attempt stays as history");
  assert!(!page.shows("merge-attempt-what"), "not as what stopped the run");
  assert!(
    page.window.text("merge-attempt-history-note").contains("merged later"),
    "the history says the run was merged later"
  );
}

#[test]
fn merge_attempt_page_merged_later_shows_the_banner_and_the_attempt_as_history_without_fix_actions() {
  let mut page = Page::new();
  let run = run_of(&page, "Per-endpoint secrets");
  let mut db = ProjectDb::open(&page.repo).expect("the project database");
  db.save_run_merge(&RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("a branch name"),
    revision: Revision::new("d08e0941d08e0941d08e0941d08e0941d08e0941").expect("a revision"),
    kind: MergeKind::FastForward,
    merged_at: Timestamp::now(),
  })
  .expect("the merge is recorded");
  drop(db);

  page.open("Per-endpoint secrets");
  assert_eq!(page.window.text("merge-attempt-status-text"), "Merged into main");
  let banner = page.window.text("merge-attempt-merged-banner");
  assert!(
    banner.contains("Merged into main at d08e094") && banner.contains("As a fast-forward"),
    "{banner}"
  );
  assert_history_without_fixes(&mut page);
  assert!(
    page.window.text("merge-attempt-conflict-0").contains("Conflicted then"),
    "the files stay as history, not as a live state"
  );
  assert_eq!(
    page.window.text("merge-attempt-property-into-value"),
    "main",
    "Into names the branch, not today's tip"
  );
}

#[test]
fn merge_attempt_page_merged_outside_orchester_shows_the_banner_and_no_fix_actions() {
  let mut page = Page::new();
  let run = run_of(&page, "Add retry to webhook sender");
  let git = |args: &[&str]| {
    let output = Command::new("git")
      .current_dir(&page.repo)
      .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
      .args(args)
      .output()
      .expect("git runs");
    assert!(output.status.success(), "git {args:?}: {output:?}");
  };
  git(&["merge", "--no-ff", "--no-edit", run.branch().as_str()]);

  page.open("Add retry to webhook sender");
  assert_eq!(
    page.window.text("merge-attempt-status-text"),
    "Merged into main outside Orchester"
  );
  let banner = page.window.text("merge-attempt-merged-banner");
  assert!(banner.contains("Merged into main outside Orchester"), "{banner}");
  assert_history_without_fixes(&mut page);
  assert_eq!(page.window.text("merge-attempt-property-into-value"), "main");
}

#[test]
fn merge_attempt_page_a_title_wider_than_its_room_ends_before_the_header_actions() {
  let mut page = Page::new();
  page.window.tree.resize(760, 1400);
  page.open("Add retry to webhook sender");
  let right = |page: &mut Page, id: &str| {
    page
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .map(|bounds| (bounds.x, bounds.x + bounds.width))
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (_, title_end) = right(&mut page, "merge-attempt-title");
  let (actions_start, _) = right(&mut page, "merge-attempt-copy-details");
  assert!(
    title_end <= actions_start,
    "the title ends at {title_end}, before the actions start at {actions_start}"
  );
}
