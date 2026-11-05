//! The direct merge's preflight (ORC-644): each stage that refuses says which it was and why, merges nothing and
//! leaves the branch and checkout as they were. `cargo test -p orchester-workspace --test local_integration
//! preflight_stage`.

use std::{fs, path::Path};

use orchester_core::integration::{PreflightCause, PreflightStage, local::LocalError};
use orchester_exec::{local_git::IntegrationLease, programs::Programs};
use orchester_workspace::tasks::integration::{IntegrationProblem, merge_in};

use super::support::{Fixture, git};

/// Merges the fixture's run, which must be refused; the refusal, after checking the checkout's branch did not move.
fn refused(fixture: &Fixture) -> IntegrationProblem {
  let before = git(fixture.root(), &["rev-parse", "HEAD"]);
  let refusal = merge_in(&fixture.worktrees(), &Programs::processes(), (fixture.run, None)).expect_err("refused");
  assert_eq!(
    git(fixture.root(), &["rev-parse", "HEAD"]),
    before,
    "nothing was merged"
  );
  refusal
}

fn preflight(stage: PreflightStage, cause: PreflightCause) -> IntegrationProblem {
  IntegrationProblem::Preflight { stage, cause }
}

#[test]
fn preflight_stage_integration_lock_held_by_another_operation_is_told_as_held() {
  let fixture = Fixture::new(false);
  let held = IntegrationLease::acquire(fixture.root()).expect("another operation's lease");
  let refusal = refused(&fixture);
  assert_eq!(
    refusal,
    preflight(PreflightStage::IntegrationLock, PreflightCause::Held)
  );
  assert_eq!(
    refusal.message("").key,
    "localIntegration.preflight.integrationLockHeld"
  );
  drop(held);
  merge_in(&fixture.worktrees(), &Programs::processes(), (fixture.run, None)).expect("merged once the lease is free");
}

#[test]
fn preflight_stage_project_database_refuses_a_linked_sidecar() {
  let fixture = Fixture::new(false);
  let store = fixture.root().join(".orchester");
  fs::write(store.join("elsewhere"), "").expect("another file");
  fs::hard_link(store.join("elsewhere"), store.join("orchester.db-journal")).expect("a second name for it");
  let refusal = refused(&fixture);
  assert_eq!(
    refusal,
    preflight(
      PreflightStage::ProjectDatabase,
      PreflightCause::State(LocalError::Unsupported)
    )
  );
  let message = refusal.message("");
  assert_eq!(message.key, "localIntegration.preflight.projectDatabase");
  assert_eq!(message.args, vec![("cause", LocalError::Unsupported.to_string())]);
}

#[test]
fn preflight_stage_scratch_folder_refuses_a_file_where_the_folder_belongs() {
  let fixture = Fixture::new(false);
  let folder = fixture.root().join(".orchester/local-integration");
  fs::create_dir_all(&folder).expect("local integration's folder");
  fs::write(folder.join("legacy"), "not a folder").expect("a file in its place");
  assert_eq!(
    refused(&fixture),
    preflight(
      PreflightStage::ScratchFolder,
      PreflightCause::State(LocalError::Unsupported)
    )
  );
}

/// A scratch folder the OS refuses to make is named, with the OS's error, relative to the project.
#[cfg(unix)]
#[test]
fn preflight_stage_scratch_folder_names_the_folder_the_os_refused() {
  use std::os::unix::fs::PermissionsExt;
  let fixture = Fixture::new(false);
  let folder = fixture.root().join(".orchester/local-integration");
  fs::create_dir_all(&folder).expect("local integration's folder");
  assert!(!folder.join("legacy").exists(), "no merge made it yet");
  fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).expect("read-only");
  let refusal = refused(&fixture);
  fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).expect("writable again");
  let IntegrationProblem::Preflight {
    stage: PreflightStage::ScratchFolder,
    cause: PreflightCause::Io { path, kind, code, text },
  } = &refusal
  else {
    panic!("{refusal:?}");
  };
  assert_eq!(path.as_deref(), Some(".orchester/local-integration/legacy"));
  assert_eq!(*kind, std::io::ErrorKind::PermissionDenied);
  assert_eq!(*code, Some(13), "EACCES");
  assert!(!text.is_empty());
  assert!(
    refusal.message("").args[0]
      .1
      .starts_with(".orchester/local-integration/legacy: "),
    "{:?}",
    refusal.message("")
  );
}

#[test]
fn preflight_stage_checkout_identity_refuses_a_work_tree_git_puts_elsewhere() {
  let fixture = Fixture::new(false);
  let elsewhere = fixture.root().join("elsewhere");
  fs::create_dir_all(&elsewhere).expect("another folder");
  git(
    fixture.root(),
    &["config", "core.worktree", path_text(&elsewhere).as_str()],
  );
  let refusal = refused(&fixture);
  assert_eq!(
    refusal,
    preflight(
      PreflightStage::CheckoutIdentity,
      PreflightCause::State(LocalError::Unsupported)
    )
  );
}

#[test]
fn preflight_stage_checkout_identity_keeps_gits_exit_and_its_last_words() {
  let fixture = Fixture::new(false);
  let before = git(fixture.root(), &["rev-parse", "HEAD"]);
  fs::rename(fixture.root().join(".git"), fixture.root().join("moved-git")).expect("move the Git directory");
  let missing = fixture.root().join("missing-git");
  fs::write(
    fixture.root().join(".git"),
    format!("gitdir: {}\n", path_text(&missing)),
  )
  .expect("a Git file naming nothing");
  let refusal = merge_in(&fixture.worktrees(), &Programs::processes(), (fixture.run, None)).expect_err("refused");
  fs::remove_file(fixture.root().join(".git")).expect("the Git file");
  fs::rename(fixture.root().join("moved-git"), fixture.root().join(".git")).expect("the Git directory back");
  assert_eq!(
    git(fixture.root(), &["rev-parse", "HEAD"]),
    before,
    "nothing was merged"
  );
  let IntegrationProblem::Preflight {
    stage: PreflightStage::CheckoutIdentity,
    cause: PreflightCause::Git(cause),
  } = &refusal
  else {
    panic!("{refusal:?}");
  };
  assert_eq!(cause.exit(), Some(128));
  assert!(cause.tail().contains("not a git repository"), "{}", cause.tail());
  assert_eq!(refusal.message("").key, "localIntegration.preflight.checkoutIdentity");
}

/// Windows holds the repository's Git metadata while Git changes it, and refuses objects borrowed from elsewhere;
/// macOS holds none of it (AGENTS.md, known gaps).
#[cfg(windows)]
#[test]
fn preflight_stage_git_metadata_refuses_borrowed_objects() {
  let fixture = Fixture::new(false);
  let info = fixture.root().join(".git/objects/info");
  fs::create_dir_all(&info).expect("objects info");
  fs::write(info.join("alternates"), "C:/elsewhere/objects\n").expect("alternates");
  let refusal = refused(&fixture);
  fs::remove_file(info.join("alternates")).expect("alternates removed");
  assert_eq!(
    refusal,
    preflight(
      PreflightStage::GitMetadata,
      PreflightCause::State(LocalError::Unsupported)
    )
  );
}

fn path_text(path: &Path) -> String {
  path.to_str().expect("a UTF-8 test path").replace('\\', "/")
}
