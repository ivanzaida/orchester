use std::{fs, path::Path};

use orchester_store::{PreviousSession, SessionEnd};
use orchester_workspace::{
  app_paths::{AppDataLocation, app_database_beside},
  project_setup::ProjectDraft,
  session::{AppSession, StartupError},
  workspace::Landing,
};
use tempfile::TempDir;

fn git_repo(parent: &Path, name: &str) -> std::path::PathBuf {
  let repo = parent.join(name);
  fs::create_dir_all(repo.join(".git")).expect("create .git");
  repo
}

#[test]
fn app_database_lives_next_to_the_executable() {
  let executable = Path::new("C:/Program Files/Orchester/orchester.exe");
  assert_eq!(
    app_database_beside(executable),
    Path::new("C:/Program Files/Orchester/orchester.db")
  );
}

#[test]
fn app_data_lives_next_to_the_executable_with_its_logs() {
  let executable = Path::new("C:/Program Files/Orchester/orchester.exe");
  let location = AppDataLocation::beside(executable);
  assert_eq!(location.database, Path::new("C:/Program Files/Orchester/orchester.db"));
  assert_eq!(location.log_dir, Path::new("C:/Program Files/Orchester/logs"));
}

#[cfg(target_os = "macos")]
mod macos {
  use std::path::Path;

  use orchester_workspace::app_paths::{AppDataLocation, macos};

  #[test]
  fn app_bundle_keeps_its_data_in_the_users_library() {
    let executable = Path::new("/Applications/Orchester.app/Contents/MacOS/orchester");
    let location = macos::location_of(executable, Some(Path::new("/Users/ada"))).expect("location");
    assert_eq!(
      location.database,
      Path::new("/Users/ada/Library/Application Support/Orchester/orchester.db")
    );
    assert_eq!(location.log_dir, Path::new("/Users/ada/Library/Logs/Orchester"));
  }

  #[test]
  fn development_build_keeps_its_data_beside_the_executable() {
    let executable = Path::new("/Users/ada/orchester/target/debug/orchester");
    let location = macos::location_of(executable, Some(Path::new("/Users/ada"))).expect("location");
    assert_eq!(location, AppDataLocation::beside(executable));
    assert_eq!(
      location.database,
      Path::new("/Users/ada/orchester/target/debug/orchester.db")
    );
  }

  #[test]
  fn only_the_program_folder_of_an_app_bundle_counts_as_bundled() {
    assert!(macos::in_app_bundle(Path::new(
      "/Users/ada/Applications/Orchester.app/Contents/MacOS/orchester"
    )));
    assert!(!macos::in_app_bundle(Path::new(
      "/Applications/Orchester.app/Contents/Resources/orchester"
    )));
    assert!(!macos::in_app_bundle(Path::new(
      "/Applications/Orchester/Contents/MacOS/orchester"
    )));
    assert!(!macos::in_app_bundle(Path::new("/orchester")));
  }

  #[test]
  fn app_bundle_without_a_home_folder_is_an_error() {
    let executable = Path::new("/Applications/Orchester.app/Contents/MacOS/orchester");
    assert!(macos::location_of(executable, None).is_err());
    assert!(macos::location_of(executable, Some(Path::new("relative"))).is_err());
  }
}

#[test]
fn startup_reports_an_unopenable_database_with_its_path() {
  let dir = TempDir::new().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  fs::create_dir(&path).expect("a folder where the database file belongs");
  let Err(error) = AppSession::start_at(&path) else {
    panic!("a folder cannot be opened as the database");
  };
  assert_eq!(error.path(), Some(path.as_path()));
  assert_eq!(error.message().key, "errors.storage");
}

#[test]
fn startup_reports_a_data_folder_it_cannot_write_with_its_path() {
  let dir = TempDir::new().expect("temporary directory");
  let folder = dir.path().join("missing");
  let Err(error) = AppSession::start_at(&folder.join("orchester.db")) else {
    panic!("no lock can be written in a missing folder");
  };
  assert!(matches!(error, StartupError::DataFolder { .. }));
  assert_eq!(error.path(), Some(folder.as_path()));
  assert_eq!(error.message().key, "errors.dataFolderNotWritable");
}

#[test]
fn startup_leaves_a_data_folder_another_app_has_open() {
  let dir = TempDir::new().expect("temporary directory");
  let database = dir.path().join("orchester.db");
  let first = AppSession::start_at(&database).expect("the first app starts");

  let Err(StartupError::InUse { folder, holder }) = AppSession::start_at(&database) else {
    panic!("a second app in the same data folder is refused");
  };
  assert_eq!(folder, dir.path());
  let holder = holder.expect("the first app's record says who it is");
  assert_eq!(holder.pid, std::process::id());
  assert_eq!(holder.started_at, first.started_at());

  first.end(SessionEnd::Clean).expect("the first app ends its session");
  let next = AppSession::start_at(&database).expect("the folder is free once the first app ended");
  assert_eq!(next.previous(), PreviousSession::Ended(SessionEnd::Clean));
}

#[test]
fn startup_lands_on_first_launch_then_on_the_created_project() {
  let dir = TempDir::new().expect("temporary directory");
  let database = dir.path().join("orchester.db");
  let repo = git_repo(dir.path(), "kontur");

  let session = AppSession::start_at(&database).expect("session starts");
  assert!(matches!(session.workspace().landing(), Ok(Landing::FirstLaunch)));
  let new = ProjectDraft {
    name: "Kontur",
    repository: Some(&repo),
  }
  .validate()
  .expect("valid draft");
  let created = session.workspace().create(new).expect("project is created");
  session.end(SessionEnd::Clean).expect("session ends");

  let session = AppSession::start_at(&database).expect("session starts again");
  let Ok(Landing::Project(opened)) = session.workspace().landing() else {
    panic!("the created project opens at the next start");
  };
  assert_eq!(opened, created);
}

#[test]
fn startup_reports_a_project_whose_repository_lost_its_data() {
  let dir = TempDir::new().expect("temporary directory");
  let database = dir.path().join("orchester.db");
  let repo = git_repo(dir.path(), "kontur");
  let session = AppSession::start_at(&database).expect("session starts");
  let new = ProjectDraft {
    name: "Kontur",
    repository: Some(&repo),
  }
  .validate()
  .expect("valid draft");
  session.workspace().create(new).expect("project is created");
  fs::remove_dir_all(repo.join(".orchester")).expect("remove project data");

  let Ok(Landing::Unavailable { project, .. }) = session.workspace().landing() else {
    panic!("a project without its database is reported as unavailable");
  };
  assert_eq!(project.repo_root, repo);
}
