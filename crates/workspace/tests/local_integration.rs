#[path = "local_integration/abort.rs"]
mod abort;
#[path = "local_integration/authority.rs"]
mod authority;
#[path = "local_integration/cleanup_edits.rs"]
mod cleanup_edits;
#[path = "local_integration/drafts.rs"]
mod drafts;
#[path = "local_integration/external.rs"]
mod external;
#[path = "local_integration/git_untouched.rs"]
mod git_untouched;
#[path = "local_integration/hooks.rs"]
mod hooks;
#[path = "local_integration/ignore_transitions.rs"]
mod ignore_transitions;
#[path = "local_integration/metadata.rs"]
mod metadata;
#[path = "local_integration/privacy.rs"]
mod privacy;
#[path = "local_integration/release_regressions.rs"]
mod release_regressions;
#[path = "local_integration/review_regressions.rs"]
mod review_regressions;
#[path = "local_integration/schema.rs"]
mod schema;
#[path = "local_integration/selection.rs"]
mod selection;
#[path = "local_integration/snapshots.rs"]
mod snapshots;
#[path = "local_integration/store_contention.rs"]
mod store_contention;
#[path = "local_integration/support.rs"]
mod support;
#[path = "local_integration/sync_refusal.rs"]
mod sync_refusal;
#[path = "local_integration/worktree_removal.rs"]
mod worktree_removal;

use std::fs;

use orchester_core::{
  integration::local::{
    LocalError, LocalIntegrationStore, LocalPath, LocalPhase, LocalRecoveryStore, Placement, RecoveryLayer, Resolution,
  },
  store::MergeStore,
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::IntegrationError;
use support::{Fixture, git};

#[test]
fn local_integration_preserve_check_merge_restore_restart_and_cleanup_have_real_outcomes() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "local staged\n").expect("edit");
  git(fixture.root(), &["add", "local.txt"]);
  fs::write(fixture.root().join("local.txt"), "local unstaged\n").expect("edit");
  fs::write(fixture.root().join("untracked.bin"), [0, 255, 1]).expect("untracked");
  fs::write(fixture.root().join("ignored.txt"), "ignored").expect("ignored");
  let mut session = fixture.open();
  session.preserve("Before test delivery").expect("preserve");
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  drop(session);
  let mut session = fixture.open();
  session.sync().expect("fresh Check");
  assert_eq!(session.operation().phase(), LocalPhase::AwaitingApproval);
  let exact = session.operation().checked().expect("checked").clone();
  session.approve_and_merge(&exact).expect("merge");
  let merged = session.operation().merged().expect("merged").to_owned();
  assert_eq!(session.operation().phase(), LocalPhase::MergedAwaitingRestore);
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("reconcile");
  assert_eq!(session.operation().merged(), Some(merged.as_str()));
  session.restore().expect("restore");
  assert_eq!(session.operation().phase(), LocalPhase::RestorationVerified);
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("working"),
    b"local unstaged\n"
  );
  assert_eq!(git(fixture.root(), &["show", ":local.txt"]), "local staged");
  assert_eq!(
    fs::read(fixture.root().join("untracked.bin")).expect("binary"),
    [0, 255, 1]
  );
  assert_eq!(
    fs::read(fixture.root().join("ignored.txt")).expect("ignored"),
    b"ignored"
  );
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), merged);
  session.remove_preservation().expect("cleanup");
  assert_eq!(session.operation().phase(), LocalPhase::Complete);
  drop(session);
  let db = ProjectDb::open(fixture.root()).expect("db");
  assert!(
    db.local_content(
      fixture.run,
      &LocalPath::new("local.txt").expect("path"),
      RecoveryLayer::Working
    )
    .expect("read")
    .is_none()
  );
  assert!(db.run_merge(fixture.run).expect("recorded").is_some());
}

#[test]
fn local_integration_failed_fresh_check_never_requests_merge_approval() {
  let fixture = Fixture::new(true);
  let mut session = fixture.open();
  session.prepare_clean().expect("clean");
  assert!(matches!(session.sync(), Err(IntegrationError::CheckFailed)));
  assert_ne!(session.operation().phase(), LocalPhase::AwaitingApproval);
  assert!(session.operation().checked().is_none());
  assert!(session.operation().merged().is_none());
}

#[test]
fn local_integration_base_movement_invalidates_exact_result_and_does_not_merge() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.prepare_clean().expect("clean");
  session.sync().expect("check");
  let exact = session.operation().checked().expect("checked").clone();
  fs::write(fixture.root().join("external.txt"), "new base\n").expect("external");
  git(fixture.root(), &["add", "external.txt"]);
  git(fixture.root(), &["commit", "-m", "external"]);
  let changed = git(fixture.root(), &["rev-parse", "HEAD"]);
  assert!(matches!(
    session.approve_and_merge(&exact),
    Err(IntegrationError::State(LocalError::Stale))
  ));
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), changed);
  assert!(session.operation().merged().is_none());
}

#[test]
fn local_integration_restore_conflict_is_distinct_and_stage_choice_does_not_repeat_merge() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("task.txt"), "preserved local\n").expect("local");
  let mut session = fixture.open();
  session.preserve("Local conflicting work").expect("preserve");
  session.sync().expect("check");
  let checked = session.operation().checked().expect("checked").clone();
  session.approve_and_merge(&checked).expect("merge");
  let merged = session.operation().merged().expect("merged").to_owned();
  session.restore().expect("restore conflict");
  assert_eq!(session.operation().phase(), LocalPhase::RestoreConflicts);
  let file = session.restore_conflicts().expect("conflicts").remove(0);
  let expected = orchester_exec::local_git::file_hash(fixture.root(), &file.path).expect("hash");
  session
    .save_restore_resolution(
      &file,
      expected.as_ref(),
      &Resolution::Text("reviewed resolution\n".into()),
      Placement::LeaveUnstaged,
    )
    .expect("resolve");
  session.finish_restore().expect("verify");
  assert_eq!(git(fixture.root(), &["show", ":task.txt"]), "delivered task");
  assert_eq!(
    fs::read(fixture.root().join("task.txt")).expect("working"),
    b"reviewed resolution\n"
  );
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), merged);
  assert!(session.approve_and_merge(&checked).is_err());
}

#[test]
fn local_integration_journal_rejects_stale_writer_and_retains_verified_originals() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "local\n").expect("local");
  let mut session = fixture.open();
  session.preserve("Retained").expect("preserve");
  let operation = session.operation().clone();
  drop(session);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  let mut changed = operation.clone();
  changed.advance_version();
  assert!(
    db.save_local_operation(&changed, Some(operation.version()))
      .expect("save")
  );
  assert!(
    !db
      .save_local_operation(&changed, Some(operation.version()))
      .expect("stale")
  );
  assert!(db.remove_local_content(fixture.run).is_err());
  assert_eq!(
    db.local_content(
      fixture.run,
      &LocalPath::new("local.txt").expect("path"),
      RecoveryLayer::Working
    )
    .expect("original")
    .expect("bytes"),
    b"local\n"
  );
}
#[test]
fn local_integration_task_text_conflicts_resume_and_run_fresh_check() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("task.txt"), "main work\n").expect("main edit");
  git(fixture.root(), &["commit", "-am", "main moved"]);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("task conflicts");
  assert_eq!(session.operation().phase(), LocalPhase::TaskConflicts);
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("restart");
  let scratch = fixture.task_root.join(".orchester/local-integration");
  let task = orchester_exec::local_git::LocalGit::new(&fixture.task_root, &scratch).expect("task");
  let file = task.conflicts().expect("conflicts").remove(0);
  let expected = orchester_exec::local_git::file_hash(&fixture.task_root, &file.path).expect("hash");
  session
    .save_task_resolution(
      &file,
      expected.as_ref(),
      &Resolution::Text("combined reviewed result\n".into()),
    )
    .expect("save");
  session.continue_task().expect("fresh check");
  assert_eq!(session.operation().phase(), LocalPhase::AwaitingApproval);
  let exact = session.operation().checked().expect("checked").clone();
  session.approve_and_merge(&exact).expect("merge");
  assert_eq!(
    fs::read(fixture.root().join("task.txt")).expect("result"),
    b"combined reviewed result\n"
  );
}

#[test]
fn local_integration_reconciles_git_merge_before_journal_write_without_repeating_merge() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("check");
  let exact = session.operation().checked().expect("checked").clone();
  let mut operation = session.operation().clone();
  operation.approve(&exact).expect("approve");
  operation.begin_merge(&exact).expect("intent");
  operation.advance_version();
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  assert!(
    db.save_local_operation(&operation, Some(session.operation().version()))
      .expect("save intent")
  );
  drop(session);
  let merge = std::process::Command::new("git")
    .args(["merge", "--ff-only", &exact.result.head])
    .env("GIT_REFLOG_ACTION", operation.merge_action().expect("owned action"))
    .current_dir(fixture.root())
    .output()
    .expect("owned private merge");
  assert!(merge.status.success());
  let mut session = fixture.open();
  assert!(matches!(
    session.refresh(),
    Err(IntegrationError::State(LocalError::WrongPhase))
  ));
  session.reconcile().expect("reconcile");
  assert_eq!(session.operation().merged(), Some(exact.result.head.as_str()));
  assert!(session.approve_and_merge(&exact).is_err());
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), exact.result.head);
  assert!(db.run_merge(fixture.run).expect("record").is_some());
}

#[test]
fn local_integration_abort_refuses_external_edits_and_keeps_named_preservation() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("task.txt"), "main work\n").expect("main edit");
  git(fixture.root(), &["commit", "-am", "main moved"]);
  fs::write(fixture.root().join("local.txt"), "local original\n").expect("local");
  let mut session = fixture.open();
  session.preserve("Before task conflicts").expect("preserve");
  session.sync().expect("task conflicts");
  fs::write(fixture.task_root.join("foreign.txt"), "external work\n").expect("external");
  assert!(session.abort_task().is_err());
  assert_eq!(
    fs::read(fixture.task_root.join("foreign.txt")).expect("retained"),
    b"external work\n"
  );
  assert!(session.operation().preservation().expect("saved").retained);
}
#[test]
fn local_integration_restart_refuses_live_process_identity_and_never_signals_it() {
  use orchester_core::integration::local::LocalProcessStore;
  let fixture = Fixture::new(false);
  let session = fixture.open();
  drop(session);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  let sequence = db.begin_local_process(fixture.run).expect("intent");
  let identity = orchester_exec::run_processes::identity::capture(std::process::id())
    .expect("identity")
    .expect("running");
  db.local_process_started(fixture.run, sequence, Some(&identity))
    .expect("record");
  assert!(orchester_workspace::tasks::local_integration::LocalIntegration::open(fixture.root(), fixture.run).is_err());
  assert!(orchester_exec::run_processes::identity::still_running(&identity).expect("still running"));
  db.finish_local_process(fixture.run, sequence).expect("finished");
  let session = fixture.open();
  assert_eq!(session.operation().phase(), LocalPhase::Review);
}
#[test]
fn local_integration_restart_completes_partially_written_task_resolution() {
  use orchester_core::integration::local::{PendingResolution, ResolutionWrite};
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("task.txt"), "main work\n").expect("edit");
  git(fixture.root(), &["commit", "-am", "main moved"]);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("conflicts");
  let task = orchester_exec::local_git::LocalGit::new(
    &fixture.task_root,
    &fixture.task_root.join(".orchester/local-integration"),
  )
  .expect("task");
  let conflict = task.conflicts().expect("conflicts").remove(0);
  let before = task.snapshot().expect("before");
  let expected = task
    .file(&conflict.path)
    .expect("file")
    .as_deref()
    .map(orchester_exec::local_git::digest);
  let bytes = b"saved before restart\n";
  let hash = orchester_exec::local_git::digest(bytes);
  let entry = orchester_core::integration::local::IndexEntry {
    mode: 0o100644,
    object: task.put_path_blob(&conflict.path, bytes).expect("blob"),
  };
  let untouched = task
    .index_fingerprint_excluding(std::slice::from_ref(&conflict.path))
    .expect("index");
  let plan = PendingResolution {
    before,
    conflict: conflict.clone(),
    untouched_index: untouched,
    writes: vec![ResolutionWrite {
      path: conflict.path.clone(),
      before: expected.clone(),
      working: Some(hash.clone()),
      entry: Some(entry),
    }],
  };
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  db.retain_local_content(
    fixture.run,
    &LocalPath::new(&format!("retained-results/{}/{}", hash.as_str(), conflict.path)).expect("key"),
    RecoveryLayer::Result,
    &hash,
    bytes,
  )
  .expect("retain result");
  let mut operation = session.operation().clone();
  operation.plan_resolution(plan).expect("plan");
  operation.advance_version();
  assert!(
    db.save_local_operation(&operation, Some(session.operation().version()))
      .expect("intent")
  );
  orchester_exec::local_git::write_regular(&fixture.task_root, &conflict.path, expected.as_ref(), Some(bytes))
    .expect("partial write");
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("replay exact index write");
  session.continue_task().expect("fresh check");
  assert_eq!(session.operation().phase(), LocalPhase::AwaitingApproval);
  assert_eq!(
    git(&fixture.task_root, &["show", "HEAD:task.txt"]),
    "saved before restart"
  );
}

#[test]
fn local_integration_restart_completes_custom_restore_intent_without_task_worktree() {
  use orchester_core::integration::local::RestoredFile;
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("task.txt"), "local work\n").expect("edit");
  let mut session = fixture.open();
  session.preserve("Local work").expect("preserve");
  session.sync().expect("check");
  let checked = session.operation().checked().expect("checked").clone();
  session.approve_and_merge(&checked).expect("merge");
  session.restore().expect("conflict");
  let file = session.restore_conflicts().expect("conflicts").remove(0);
  let bytes = b"custom resolution retained\n";
  let hash = orchester_exec::local_git::digest(bytes);
  let before = orchester_exec::local_git::file_hash(fixture.root(), &file.path).expect("hash");
  let main =
    orchester_exec::local_git::LocalGit::new(fixture.root(), &fixture.root().join(".orchester/local-integration"))
      .expect("main");
  let staged = main.tree_entries("HEAD").expect("head").get(&file.path).cloned();
  let restored = file.path.clone();
  let mut operation = session.operation().clone();
  operation
    .restored_file(RestoredFile {
      path: file.path,
      before,
      executable: false,
      working: Some(hash.clone()),
      staged,
    })
    .expect("intent");
  operation.advance_version();
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  db.retain_local_content(
    fixture.run,
    &LocalPath::new(&format!("retained-results/{}/{}", hash.as_str(), restored)).expect("key"),
    RecoveryLayer::Result,
    &hash,
    bytes,
  )
  .expect("retain");
  assert!(
    db.save_local_operation(&operation, Some(session.operation().version()))
      .expect("save")
  );
  drop(session);
  git(
    fixture.root(),
    &["worktree", "remove", fixture.task_root.to_str().expect("path")],
  );
  let mut session = fixture.open();
  session.reconcile().expect("reconcile recorded merge");
  session.restore().expect("restore custom result");
  assert_eq!(session.operation().phase(), LocalPhase::RestorationVerified);
  assert_eq!(fs::read(fixture.root().join("task.txt")).expect("custom result"), bytes);
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), checked.result.head);
}
#[test]
fn local_integration_cancel_restores_exact_original_staging_without_merging_task() {
  let fixture = Fixture::new(false);
  let original_head = git(fixture.root(), &["rev-parse", "HEAD"]);
  fs::write(fixture.root().join("local.txt"), "staged local\n").expect("edit");
  git(fixture.root(), &["add", "local.txt"]);
  fs::write(fixture.root().join("local.txt"), "working local\n").expect("edit");
  fs::write(fixture.root().join("untracked.bin"), [0, 99]).expect("untracked");
  let mut session = fixture.open();
  session.preserve("Cancel recovery").expect("preserve");
  session.cancel_preservation().expect("cancel and restore");
  assert_eq!(session.operation().phase(), LocalPhase::Canceled);
  assert_eq!(git(fixture.root(), &["show", ":local.txt"]), "staged local");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("working"),
    b"working local\n"
  );
  assert_eq!(
    fs::read(fixture.root().join("untracked.bin")).expect("untracked"),
    [0, 99]
  );
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), original_head);
  assert!(session.operation().merged().is_none());
  session.remove_preservation().expect("verified cleanup");
  session
    .refresh()
    .expect("review restored changes for a later integration");
  assert_eq!(session.operation().phase(), LocalPhase::Review);
}

#[test]
fn local_integration_owned_abort_can_resume_preserved_preparation_after_restart() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("task.txt"), "main change\n").expect("edit");
  git(fixture.root(), &["commit", "-am", "main moved"]);
  fs::write(fixture.root().join("local.txt"), "kept local\n").expect("local");
  let mut session = fixture.open();
  session.preserve("Before abort").expect("preserve");
  session.sync().expect("task conflicts");
  session.abort_task().expect("owned abort");
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("resume prepared");
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  session.cancel_preservation().expect("restore local without merge");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("local"),
    b"kept local\n"
  );
  assert!(session.operation().merged().is_none());
}
#[test]
fn local_integration_restart_finishes_preservation_before_any_checkout_clear() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "retained original\n").expect("local");
  let session = fixture.open();
  let mut operation = session.operation().clone();
  operation
    .begin_preserve(&operation.reviewed().clone(), "Interrupted retention")
    .expect("intent");
  operation.advance_version();
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  assert!(
    db.save_local_operation(&operation, Some(session.operation().version()))
      .expect("save")
  );
  drop(session);
  let mut session = fixture.open();
  session
    .reconcile()
    .expect("complete verified retention and scoped clear");
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  session.cancel_preservation().expect("restore verified original");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("original"),
    b"retained original\n"
  );
}
#[test]
fn local_integration_cancel_can_recover_preservation_after_task_worktree_removal() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "local originals\n").expect("edit");
  let mut session = fixture.open();
  session.preserve("Before removal").expect("preserve");
  drop(session);
  git(
    fixture.root(),
    &["worktree", "remove", fixture.task_root.to_str().expect("path")],
  );
  let mut session = fixture.open();
  session.cancel_preservation().expect("restore without task checkout");
  assert_eq!(session.operation().phase(), LocalPhase::Canceled);
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("original"),
    b"local originals\n"
  );
  assert!(session.operation().merged().is_none());
}
#[test]
fn local_integration_newer_run_invalidates_old_run_merge_authority() {
  use orchester_core::{
    id::IdempotencyKey,
    run::{BranchName, NewRun},
    store::RunStore,
    time::Timestamp,
  };
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("check");
  let checked = session.operation().checked().expect("exact").clone();
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  let run = db.run(fixture.run).expect("run").expect("exists");
  db.create_run(
    IdempotencyKey::generate(),
    NewRun {
      task: run.task(),
      snapshot: run.snapshot().clone(),
      branch: BranchName::new("later-task").expect("branch"),
      base: run.base().clone(),
      base_branch: run.base_branch().cloned(),
      budget: None,
      max_duration: None,
    },
    Timestamp::now(),
  )
  .expect("later run");
  assert!(session.approve_and_merge(&checked).is_err());
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), checked.base.head);
  assert!(db.run_merge(fixture.run).expect("merge record").is_none());
}
