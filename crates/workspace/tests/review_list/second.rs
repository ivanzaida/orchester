//! A second task of the review tests' project, whose run also passed every step: it adds `src/two.rs` on its own
//! branch, so it merges beside the first run's work.

use std::fs;

use orchester_core::{
  id::{IdempotencyKey, RunId},
  revision::Revision,
  run::{BranchName, NewRun},
  store::{RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
};

use super::support::{Repo, at, git};

/// Run 2 of `repo`'s project, created at `millis` and passed every step two milliseconds later; its id.
pub fn run(repo: &Repo, millis: i64) -> RunId {
  let mut db = repo.db();
  let first = repo.stored();
  let task = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![TaskDetails {
        title: orchester_core::name::Name::new("Add letters").expect("name"),
        brief: String::new(),
        acceptance_criteria: Vec::new(),
        pipeline: None,
        limits: TaskLimits::NONE,
      }],
      at(millis),
    )
    .expect("task")
    .remove(0);
  let mut run = db
    .create_run(
      IdempotencyKey::generate(),
      NewRun {
        task: task.id(),
        snapshot: first.snapshot().clone(),
        branch: BranchName::new("orchester/rev-2").expect("branch"),
        base: first.base().clone(),
        base_branch: first.base_branch().cloned(),
        base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
        budget: None,
        max_duration: None,
      },
      at(millis),
    )
    .expect("run");
  let events = run.start(at(millis + 1)).expect("start");
  db.append_events(&run, &events).expect("events");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, at(millis + 1)).expect("step");
  db.append_events(&run, &events).expect("events");
  let worktree = repo.root().join(".orchester/worktrees").join(run.id().to_string());
  git(
    repo.root(),
    &[
      "worktree",
      "add",
      "-b",
      "orchester/rev-2",
      worktree.to_str().expect("path"),
    ],
  );
  fs::write(worktree.join("src/two.rs"), "fn letters() {}\n").expect("write");
  git(&worktree, &["add", "."]);
  git(&worktree, &["commit", "-m", "letters"]);
  let commit = Revision::new(&git(&worktree, &["rev-parse", "HEAD"])).expect("revision");
  let events = run
    .record_checkpoint(execution, commit, at(millis + 2))
    .expect("checkpoint");
  db.append_events(&run, &events).expect("events");
  let events = run.complete_step(execution, at(millis + 2)).expect("passed");
  db.append_events(&run, &events).expect("events");
  run.id()
}

/// Where `run`'s branch is.
pub fn head(repo: &Repo, run: RunId) -> Revision {
  let branch = repo.db().run(run).expect("read").expect("the run").branch().clone();
  Revision::new(&git(repo.root(), &["rev-parse", branch.as_str()])).expect("revision")
}
