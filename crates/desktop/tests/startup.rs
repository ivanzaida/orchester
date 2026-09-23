use std::{fs, path::Path};

use orchester_desktop::{
  app_paths::app_database_beside, project_setup::ProjectDraft, session::AppSession, workspace::Landing,
};
use orchester_store::SessionEnd;
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
fn startup_reports_an_unopenable_database_with_its_path() {
  let dir = TempDir::new().expect("temporary directory");
  let path = dir.path().join("missing").join("orchester.db");
  let Err(error) = AppSession::start_at(&path) else {
    panic!("a database in a missing directory cannot be opened");
  };
  assert_eq!(error.path(), Some(path.as_path()));
  assert_eq!(error.message().key, "errors.storage");
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
