//! A run whose branch merged in another branch than its target (`orchester_core::integration::TakenIn`): its changes
//! say how many commits it took in and from where, its agent's own work reads alone, and `merge_run` leaves its merge
//! to the person.

use std::path::Path;

use orchester_core::store::RunStore;
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::changes::{Checkpointed, RunRange, run_changes_view_in};

use super::support::{Fixture, git};

fn commit(root: &Path, file: &str, content: &str, message: &str) -> String {
  std::fs::write(root.join(file), content).expect("write");
  git(root, &["add", file]);
  git(root, &["commit", "-m", message]);
  git(root, &["rev-parse", "HEAD"])
}

/// A run that wrote its own file, then merged in `other`, a branch of three commits main does not have.
fn took_in_other(prefix: &str) -> Fixture {
  Fixture::worked(false, (prefix, "task"), |root, task| {
    let own = commit(task, "task.txt", "the run's work\n", "task work");
    git(root, &["checkout", "-q", "-b", "other", "main"]);
    for n in 0..3 {
      commit(root, &format!("other-{n}.txt"), "other work\n", &format!("other {n}"));
    }
    git(root, &["checkout", "-q", "main"]);
    git(task, &["merge", "--no-ff", "--no-edit", "other"]);
    let merged = git(task, &["rev-parse", "HEAD"]);
    vec![own, merged]
  })
}

fn files(checkpointed: &Checkpointed) -> Vec<String> {
  let Checkpointed::Read { changes, .. } = checkpointed else {
    panic!("read: {checkpointed:?}");
  };
  let mut paths: Vec<String> = changes.diff.files().iter().map(|file| file.path().to_owned()).collect();
  paths.sort();
  paths
}

#[test]
fn taken_in_says_what_the_branch_took_in_and_reads_the_run_s_own_changes_alone() {
  let fixture = took_in_other("taken-in-");
  let run = ProjectDb::open(fixture.root())
    .expect("db")
    .run(fixture.run)
    .expect("read")
    .expect("run");
  let range = RunRange::of(&run);
  let programs = Programs::processes();

  let all = run_changes_view_in(fixture.root(), &programs, (&range, false)).expect("changes");
  let taken = all.taken_in.clone().expect("what it took in");
  assert_eq!(taken.commits, 3, "the three commits of other");
  assert_eq!(taken.from.as_ref().map(|branch| branch.as_str()), Some("other"));
  assert!(!all.own_only);
  assert_eq!(
    files(&all.checkpointed),
    ["other-0.txt", "other-1.txt", "other-2.txt", "task.txt"],
    "everything the merge brings"
  );

  let own = run_changes_view_in(fixture.root(), &programs, (&range, true)).expect("own changes");
  assert!(own.own_only);
  assert_eq!(files(&own.checkpointed), ["task.txt"], "the agent's work alone");
}
