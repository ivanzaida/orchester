//! What the steps of a merge's preflight keep of a failure (ORC-644): the lease tells another operation holding it
//! apart from any other failure, and an OS or Git failure keeps its path, error and Git's last words.
//! `cargo test -p orchester-exec --test local_integration preflight_stage`.

use std::{fs, io::ErrorKind};

use orchester_core::integration::{PreflightCause, local::LocalError};
use orchester_exec::local_git::{IntegrationLease, LocalGit, LocalGitError, owned::StoreOwnership};

use super::Repo;

#[test]
fn preflight_stage_lease_held_by_another_operation_is_contention() {
  let repo = Repo::new();
  let held = IntegrationLease::acquire(repo.root()).expect("the first operation's lease");
  assert!(matches!(
    IntegrationLease::acquire(repo.root()),
    Err(LocalGitError::State(LocalError::ForeignOperation))
  ));
  drop(held);
  IntegrationLease::acquire(repo.root()).expect("taken once it is free");
}

#[test]
fn preflight_stage_lease_that_cannot_be_a_file_is_not_contention() {
  let repo = Repo::new();
  fs::create_dir_all(repo.root().join(".orchester/local-integration.lock")).expect("a folder in its place");
  let refused = IntegrationLease::acquire(repo.root()).err().expect("refused");
  assert!(
    matches!(refused, LocalGitError::State(LocalError::Unsupported)),
    "{refused:?}"
  );
}

#[test]
fn preflight_stage_database_sidecar_with_a_second_name_is_refused() {
  let repo = Repo::new();
  let store = repo.root().join(".orchester");
  fs::write(store.join("orchester.db"), "").expect("database");
  fs::write(store.join("elsewhere"), "").expect("another file");
  fs::hard_link(store.join("elsewhere"), store.join("orchester.db-wal")).expect("a second name for it");
  assert!(matches!(
    StoreOwnership::acquire(repo.root()),
    Err(LocalGitError::State(LocalError::Unsupported))
  ));
}

#[test]
fn preflight_stage_missing_database_names_it() {
  let repo = Repo::new();
  let Err(refused) = StoreOwnership::acquire(repo.root()) else {
    panic!("a project without its database is held");
  };
  let PreflightCause::Io { path, kind, .. } = refused.preflight_cause(repo.root()) else {
    panic!("{refused:?}");
  };
  assert_eq!(path.as_deref(), Some(".orchester/orchester.db"));
  assert_eq!(kind, ErrorKind::NotFound);
}

#[test]
fn preflight_stage_missing_scratch_folder_names_it_with_the_os_error() {
  let repo = Repo::new();
  let missing = repo.root().join(".orchester/missing");
  let Err(refused) = LocalGit::new(repo.root(), &missing) else {
    panic!("a scratch folder that is not there is used");
  };
  let LocalGitError::Io(cause) = &refused else {
    panic!("{refused:?}");
  };
  assert_eq!(cause.kind(), ErrorKind::NotFound);
  assert!(!cause.text().is_empty());
  let PreflightCause::Io { path, .. } = refused.preflight_cause(repo.root()) else {
    panic!("{refused:?}");
  };
  assert_eq!(path.as_deref(), Some(".orchester/missing"));
}

#[test]
fn preflight_stage_checkout_identity_refuses_a_folder_inside_the_checkout() {
  let repo = Repo::new();
  let inside = repo.root().join("inside");
  fs::create_dir_all(inside.join(".orchester/scratch")).expect("a folder of the checkout");
  assert!(matches!(
    LocalGit::new(&inside, &inside.join(".orchester/scratch")),
    Err(LocalGitError::State(LocalError::Unsupported))
  ));
}

#[test]
fn preflight_stage_git_refusal_keeps_its_exit_code_and_last_words() {
  // The checkout opened with it holds its Git directory, which Windows then does not let move.
  let Repo { git: opened, dir } = Repo::new();
  drop(opened);
  let root = dir.path();
  fs::rename(root.join(".git"), root.join("moved-git")).expect("move the Git directory");
  let missing = root.join("missing-git");
  fs::write(
    root.join(".git"),
    format!("gitdir: {}\n", missing.to_str().expect("UTF-8").replace('\\', "/")),
  )
  .expect("a Git file naming nothing");
  let refused = LocalGit::new(root, &root.join(".orchester/scratch")).err();
  let Some(LocalGitError::Refused(cause)) = &refused else {
    panic!("{refused:?}");
  };
  assert_eq!(cause.exit(), Some(128));
  assert!(cause.tail().contains("not a git repository"), "{}", cause.tail());
  assert!(cause.tail().lines().count() <= 4);
}
