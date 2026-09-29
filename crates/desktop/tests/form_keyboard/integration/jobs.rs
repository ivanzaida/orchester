//! Run integration operations retained by the project feed while UI controls unmount or race.

use orchester_desktop::{
  tabs::feed::ProjectFeeds,
  tasks::{
    integration::leftovers_in,
    integration_jobs::{JobState, MergeArgs, RemoveArgs},
  },
  workspace::OpenProject,
};
use orchester_store::ProjectDb;

use super::*;

#[test]
fn final_merge_state_is_published_before_the_operation_unlocks() {
  let (programs, gate) = blocked_git(&["worktree", "remove"]);
  let mut desktop = finished_run_with_programs(programs.clone());
  let run = latest_run(desktop.repo()).expect("run");
  let db = ProjectDb::open_read_only(desktop.repo()).expect("project database");
  let open = OpenProject {
    project: db.project().clone(),
    repo_root: desktop.repo().to_path_buf(),
  };
  let jobs = ProjectFeeds::default().of(open.project.id()).integration;
  let job = jobs.of(run.id());
  let args = MergeArgs {
    open,
    task: run.task(),
    key: "KON-1".to_owned(),
    number: run.number().get(),
    branch: "main".to_owned(),
    run: run.id(),
    programs,
    activity: desktop.activity.clone(),
    separator: Arc::from(", "),
    changed: Arc::new(|| {}),
  };
  job.start_merge(args.clone());
  desktop.wait_until("merge reaches final cleanup", |_| gate.entered.load(Ordering::SeqCst));
  let retry = job.clone();
  let after_completion = args.clone();
  let stop = Arc::new(AtomicBool::new(false));
  let stop_retry = stop.clone();
  let attempts = Arc::new(AtomicUsize::new(0));
  let retry_attempts = attempts.clone();
  let retries = thread::spawn(move || {
    while !stop_retry.load(Ordering::SeqCst) {
      let state = retry.merge().get_untracked();
      if state == JobState::Running && !retry.busy() {
        assert_ne!(
          retry.merge().get_untracked(),
          JobState::Running,
          "a running merge must retain its operation guard"
        );
      }
      retry.start_merge(args.clone());
      retry_attempts.fetch_add(1, Ordering::SeqCst);
      thread::yield_now();
    }
  });
  desktop.wait_until("stale clicks contend with running merge", |_| {
    attempts.load(Ordering::SeqCst) > 0
  });
  gate.release();
  desktop.wait_until("merge finishes", |_| job.merge().get_untracked() == JobState::Succeeded);
  stop.store(true, Ordering::SeqCst);
  retries.join().expect("retry thread");
  job.start_merge(after_completion);
  assert_eq!(
    gate.calls.load(Ordering::SeqCst),
    1,
    "no second cleanup from a stale click"
  );
}

#[test]
fn removing_worktree_stays_pending_after_its_control_unmounts() {
  let (programs, gate) = blocked_git(&["worktree", "remove"]);
  let mut desktop = finished_run_with_programs(programs.clone());
  let run = latest_run(desktop.repo()).expect("run");
  let db = ProjectDb::open_read_only(desktop.repo()).expect("project database");
  let open = OpenProject {
    project: db.project().clone(),
    repo_root: desktop.repo().to_path_buf(),
  };
  let confirmed = leftovers_in(desktop.repo(), &programs, &run).expect("leftovers").0;
  let jobs = ProjectFeeds::default().of(open.project.id()).integration;
  let job = jobs.of(run.id());
  let args = RemoveArgs {
    open,
    task: run.task(),
    key: "KON-1".to_owned(),
    number: run.number().get(),
    run: run.clone(),
    confirmed,
    programs,
    activity: desktop.activity.clone(),
    separator: Arc::from(", "),
    changed: Arc::new(|| {}),
  };
  job.start_remove(args.clone());
  desktop.wait_until("worktree removal starts", |_| gate.entered.load(Ordering::SeqCst));
  drop(job);
  let remounted = jobs.of(run.id());
  assert_eq!(remounted.remove().get_untracked(), JobState::Running);
  remounted.start_remove(args);
  gate.release();
  desktop.wait_until("worktree removal completes", |_| {
    remounted.remove().get_untracked() == JobState::Succeeded
  });
  assert_eq!(
    gate.calls.load(Ordering::SeqCst),
    1,
    "remount must not start a second cleanup"
  );
  assert!(!run_worktree(&desktop).exists());
}
