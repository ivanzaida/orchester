//! Warm workspaces (ORC-114): borrowing and giving back through the project database, waits, stranded leftovers and
//! the workspace settings.

mod support;

use std::num::NonZeroU32;

use orchester_core::{
  run::BranchName,
  store::{RunStore, WorkspaceStore},
  workspaces::{
    Borrow, Holder, KeepFree, Stranded, WaitReason, WarmChoice, WarmFolder, WarmPattern, WarmUpCommand, Warmth,
    WorkspaceNumber, WorkspaceSettings,
  },
};
use orchester_store::ProjectDb;

use self::support::{at, started_run};

fn number(n: u32) -> WorkspaceNumber {
  WorkspaceNumber::new(n).expect("above zero")
}

fn limit(n: u32) -> NonZeroU32 {
  NonZeroU32::new(n).expect("above zero")
}

#[test]
fn workspaces_borrow_records_the_holder_and_giving_back_makes_it_warm() {
  let (_repo, mut db, run, _execution) = started_run();
  let holder = Holder::Run { run: run.id() };
  let branch = BranchName::new("orchester/kon-1-run-1").expect("a branch");
  assert_eq!(
    db.borrow_workspace(holder, Some(&branch), limit(2), at(1))
      .expect("borrow"),
    Borrow::Take(number(1))
  );
  let held = db.workspaces().expect("read");
  assert_eq!(held[0].holder, Some(holder));
  assert_eq!(held[0].last_run, Some(run.id()));
  assert_eq!(held[0].branch, Some(branch));
  assert_eq!(db.return_workspace(holder, at(2)).expect("give back"), Some(number(1)));
  let idle = db.workspaces().expect("read");
  assert_eq!((idle[0].holder, idle[0].warmth), (None, Warmth::Warm));
  assert_eq!(idle[0].last_used, Some(at(2)));
  assert_eq!(db.return_workspace(holder, at(3)).expect("nothing held"), None);
}

#[test]
fn workspaces_borrow_waits_and_records_since_when_until_one_is_taken() {
  let (_repo, mut db, run, _execution) = started_run();
  db.borrow_workspace(Holder::WarmUp, None, limit(1), at(1))
    .expect("warm-up takes the only one");
  let holder = Holder::Run { run: run.id() };
  assert_eq!(
    db.borrow_workspace(holder, None, limit(1), at(2)).expect("wait"),
    Borrow::Wait(WaitReason { in_use: 1, of: 1 })
  );
  db.borrow_workspace(holder, None, limit(1), at(5)).expect("still waits");
  assert_eq!(db.workspace_waits().expect("read"), [(run.id(), at(2))]);
  db.return_workspace(Holder::WarmUp, at(6)).expect("give back");
  assert_eq!(
    db.borrow_workspace(holder, None, limit(1), at(7)).expect("borrow"),
    Borrow::Take(number(1))
  );
  assert!(db.workspace_waits().expect("read").is_empty());
}

#[test]
fn workspaces_stranded_leftovers_keep_others_out_until_resolved() {
  let (_repo, mut db, run, _execution) = started_run();
  db.borrow_workspace(Holder::WarmUp, None, limit(1), at(1))
    .expect("borrow");
  db.return_workspace(Holder::WarmUp, at(2)).expect("give back");
  let stranded = Stranded {
    run: Some(run.id()),
    files: 14,
  };
  db.set_workspace_stranded(number(1), Some(stranded)).expect("strand");
  assert_eq!(db.workspaces().expect("read")[0].stranded, Some(stranded));
  assert!(matches!(
    db.borrow_workspace(Holder::WarmUp, None, limit(1), at(3))
      .expect("wait"),
    Borrow::Wait(_)
  ));
  db.set_workspace_stranded(number(1), None).expect("resolve");
  assert_eq!(
    db.borrow_workspace(Holder::WarmUp, None, limit(1), at(4))
      .expect("borrow"),
    Borrow::Take(number(1))
  );
}

#[test]
fn workspaces_a_new_runner_gives_back_what_the_last_one_held_but_rechecks() {
  let (_repo, mut db, run, _execution) = started_run();
  let other = db.run(run.id()).expect("read").expect("the run");
  db.borrow_workspace(Holder::Run { run: other.id() }, None, limit(3), at(1))
    .expect("borrow");
  db.borrow_workspace(Holder::Recheck { run: other.id() }, None, limit(3), at(1))
    .expect("borrow");
  assert_eq!(db.return_runner_workspaces(at(2)).expect("give back"), [number(1)]);
  let records = db.workspaces().expect("read");
  assert_eq!(records[0].holder, None);
  assert_eq!(records[1].holder, Some(Holder::Recheck { run: other.id() }));
}

#[test]
fn workspaces_settings_round_trip_and_default_to_150_gb() {
  let (repo, mut db, _run, _execution) = started_run();
  assert_eq!(db.workspace_settings().expect("read"), WorkspaceSettings::default());
  assert!(
    !db.workspace_settings().expect("read").enabled,
    "workspaces are off until turned on"
  );
  let settings = WorkspaceSettings {
    enabled: true,
    keep_free: KeepFree::from_gigabytes(200).expect("200 GB"),
    warm_up: Some(WarmUpCommand::new("cargo build --tests").expect("a command")),
    folders: vec![
      WarmFolder::new(WarmPattern::new("target").expect("a pattern"), WarmChoice::Keep),
      WarmFolder {
        last_seen: Some(at(9)),
        ..WarmFolder::new(WarmPattern::new("dist").expect("a pattern"), WarmChoice::Clean)
      },
    ],
  };
  db.save_workspace_settings(&settings).expect("save");
  drop(db);
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.workspace_settings().expect("read"), settings);
}
