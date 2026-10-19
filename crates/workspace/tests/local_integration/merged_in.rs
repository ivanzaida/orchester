//! Main merged into a run's branch after it started (*Resolve in editor*, or an agent the work was returned to):
//! main's own commits are not the run's work integrated and the run merges; the merged-in base is recorded and wins
//! over an older rebase's; a merge that would hide the run's own commits main already has proves nothing.

use std::{fs, path::Path};

use orchester_core::integration::local::RebaseStore;
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{integration::merge_in, local_integration::external_integration};

use super::support::{Fixture, git};

/// Commits `file` with `content` in `root`; the commit.
fn commit(root: &Path, file: &str, content: &str, message: &str) -> String {
  fs::write(root.join(file), content).expect("write");
  git(root, &["add", file]);
  git(root, &["commit", "-m", message]);
  git(root, &["rev-parse", "HEAD"])
}

fn rebase_of(fixture: &Fixture) -> Option<orchester_core::integration::local::RunRebase> {
  ProjectDb::open(fixture.root())
    .expect("db")
    .run_rebase(fixture.run)
    .expect("read")
}

#[test]
fn local_integration_main_merged_into_the_run_is_not_integrated_and_merges() {
  let fixture = Fixture::recorded(false, "merged-in-", "task", Some("delivered task\n"));
  let checkpoint = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  let main = commit(fixture.root(), "later.txt", "later main work\n", "later main work");
  git(&fixture.task_root, &["merge", "--no-ff", "--no-edit", "main"]);
  let tip = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  assert_eq!(
    external_integration(fixture.root(), fixture.run).expect("evidence"),
    None,
    "main's own commit is not the run's work"
  );
  let recorded = rebase_of(&fixture).expect("the merged-in base is recorded");
  assert_eq!(recorded.base().as_str(), main);
  let held = recorded.checkpoint().expect("the merge holds the checkpoint");
  assert_eq!(
    (held.replaces.as_str(), held.tip.as_str()),
    (checkpoint.as_str(), tip.as_str())
  );
  let merged = merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  assert_eq!(merged.merge.revision.as_str(), tip);
  assert_eq!(git(fixture.root(), &["rev-parse", "main"]), tip);
}

#[test]
fn local_integration_a_merged_in_base_wins_over_an_older_rebase() {
  let fixture = Fixture::recorded(false, "merged-in-", "task", Some("delivered task\n"));
  let onto = commit(fixture.root(), "later.txt", "later main work\n", "later main work");
  git(&fixture.task_root, &["rebase", "main"]);
  assert_eq!(
    external_integration(fixture.root(), fixture.run).expect("evidence"),
    None
  );
  assert_eq!(rebase_of(&fixture).expect("the rebase").base().as_str(), onto);
  let newer = commit(fixture.root(), "newer.txt", "newer main work\n", "newer main work");
  git(&fixture.task_root, &["merge", "--no-ff", "--no-edit", "main"]);
  assert_eq!(
    external_integration(fixture.root(), fixture.run).expect("evidence"),
    None,
    "neither main commit is the run's work"
  );
  assert_eq!(
    rebase_of(&fixture).expect("the merged-in base").base().as_str(),
    newer,
    "the reflog still proves the older rebase; the merge's base is newer and wins"
  );
}

#[test]
fn local_integration_a_merge_of_main_that_has_the_runs_work_proves_nothing() {
  let fixture = Fixture::recorded(false, "merged-in-", "task", Some("delivered task\n"));
  // Main took the run's checkpoint already, then the run merged main back: its work is integrated in part.
  git(fixture.root(), &["merge", "--no-ff", "--no-edit", "task"]);
  git(&fixture.task_root, &["merge", "--no-ff", "--no-edit", "main"]);
  assert!(
    external_integration(fixture.root(), fixture.run)
      .expect("evidence")
      .is_some(),
    "main has the run's work"
  );
  assert_eq!(rebase_of(&fixture), None);
}
