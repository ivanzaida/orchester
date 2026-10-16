//! Projects per Orchester Cloud account (the person's decision, 2026-10-16): on one application database, each account
//! signed in sees, opens and forgets only its own projects; the projects added while no account was signed in go to
//! the first one that signs in; and the session's activity keeps nothing of another account's projects, until that
//! account signs in again.
//!
//! `cargo test -p orchester-workspace --test projects_per_account per_account_`

#[path = "sharing_support/account_cloud.rs"]
mod account_cloud;
#[path = "sharing_support/desktop.rs"]
mod desktop;
#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use std::{
  fs,
  path::{Path, PathBuf},
  time::Duration,
};

use account_cloud::{AccountCloud, EMAIL, OTHER_EMAIL, PASSWORD};
use desktop::Desktop;
use orchester_api::Credential;
use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
};
use orchester_store::StoreError;
use orchester_workspace::{
  activity::{Activity, Place, Target},
  messages::Message,
  project_setup::NewProject,
  sharing::cloud::keep_activity_to_the_account,
  workspace::{Landing, Workspace},
};
use tempfile::TempDir;

/// `name` set up in its own repository under `repos`, as the signed-in account's; its folder.
fn add(workspace: &Workspace, repos: &TempDir, name: &str) -> (ProjectId, PathBuf) {
  let repo = repos.path().join(name);
  fs::create_dir_all(repo.join(".git")).expect("a repository");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("a key"),
    Name::new(name).expect("a name"),
  );
  let id = project.id();
  workspace
    .create(NewProject {
      project,
      repo_root: repo.clone(),
    })
    .expect("the project is set up");
  (id, repo)
}

fn names(workspace: &Workspace) -> Vec<String> {
  workspace
    .projects()
    .expect("the list")
    .iter()
    .map(|project| project.name.as_str().to_owned())
    .collect()
}

/// Signs `desktop` out, then in as `email`.
fn switch_to(desktop: &Desktop, email: &str) {
  desktop.block_on(desktop.cloud.sign_out()).expect("signed out");
  desktop
    .block_on(desktop.cloud.sign_in(email.into(), Credential::new(PASSWORD.into())))
    .expect("signed in");
}

#[test]
fn per_account_each_sees_only_its_own_projects() {
  let (cloud, repos) = (AccountCloud::start(), TempDir::new().expect("repositories"));
  let desktop = Desktop::signed_in(&cloud);
  let workspace = desktop.workspace.clone();
  let (kontur, kontur_root) = add(&workspace, &repos, "Kontur");
  assert_eq!(names(&workspace), ["Kontur"]);

  switch_to(&desktop, OTHER_EMAIL);
  assert_eq!(
    names(&workspace),
    Vec::<String>::new(),
    "another account sees none of them"
  );
  assert!(workspace.entries().expect("entries").is_empty());
  assert!(matches!(workspace.landing(), Ok(Landing::FirstLaunch)));
  assert!(
    matches!(workspace.open(&kontur_root), Err(StoreError::ProjectOfAnotherAccount(id)) if id == kontur),
    "opening the folder is refused"
  );
  workspace.forget(kontur).expect("nothing to forget");
  let (_, ledger_root) = add(&workspace, &repos, "Ledger");
  assert_eq!(names(&workspace), ["Ledger"]);
  assert_eq!(
    workspace.unseen_roots().expect("unseen"),
    [kontur_root].into_iter().collect()
  );

  switch_to(&desktop, EMAIL);
  assert_eq!(
    names(&workspace),
    ["Kontur"],
    "the first account finds its project again"
  );
  assert!(matches!(workspace.landing(), Ok(Landing::Project(open)) if open.project.id() == kontur));
  assert_eq!(
    workspace.unseen_roots().expect("unseen"),
    [ledger_root].into_iter().collect()
  );
}

#[test]
fn per_account_projects_added_signed_out_go_to_the_first_account_to_sign_in() {
  let (cloud, repos) = (AccountCloud::start(), TempDir::new().expect("repositories"));
  let desktop = Desktop::new(&cloud.url());
  let workspace = desktop.workspace.clone();
  add(&workspace, &repos, "Notes");
  assert_eq!(names(&workspace), ["Notes"], "nobody's yet");

  desktop
    .block_on(
      desktop
        .cloud
        .sign_in(OTHER_EMAIL.into(), Credential::new(PASSWORD.into())),
    )
    .expect("signed in");
  assert_eq!(names(&workspace), ["Notes"], "the first account took it");
  desktop.block_on(desktop.cloud.sign_out()).expect("signed out");
  assert_eq!(
    names(&workspace),
    Vec::<String>::new(),
    "signed out, it is nobody's to see"
  );
  desktop
    .block_on(desktop.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())))
    .expect("signed in");
  assert_eq!(names(&workspace), Vec::<String>::new(), "it stays the first account's");
}

/// An event of the project in `root`.
fn record(activity: &Activity, root: &Path, name: &str) {
  activity.record(
    Some(name.into()),
    Message::new("statusBar.activity.runnerReconnected"),
    Target::Project {
      repo_root: root.to_path_buf(),
      place: Place::Overview,
    },
  );
}

fn events_of(activity: &Activity, name: &str) -> usize {
  activity
    .snapshot()
    .recent()
    .iter()
    .filter(|event| event.project() == Some(name))
    .count()
}

/// Waits until `done` holds of `activity`.
fn until(desktop: &Desktop, activity: &Activity, what: &str, done: impl Fn(&Activity) -> bool) {
  desktop.block_on(async {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !done(activity) {
      assert!(tokio::time::Instant::now() < deadline, "timed out waiting for {what}");
      tokio::time::sleep(Duration::from_millis(10)).await;
    }
  });
}

#[test]
fn per_account_activity_keeps_nothing_of_another_accounts_projects() {
  let (cloud, repos) = (AccountCloud::start(), TempDir::new().expect("repositories"));
  let desktop = Desktop::signed_in(&cloud);
  let (_, kontur_root) = add(&desktop.workspace, &repos, "Kontur");
  let activity = Activity::new();
  let following = desktop
    .runtime
    .spawn(keep_activity_to_the_account(desktop.cloud.clone(), activity.clone()));
  record(&activity, &kontur_root, "Kontur");
  until(&desktop, &activity, "Kontur's event", |activity| {
    events_of(activity, "Kontur") == 1
  });

  switch_to(&desktop, OTHER_EMAIL);
  until(&desktop, &activity, "Kontur's events to go", |activity| {
    events_of(activity, "Kontur") == 0
  });
  record(&activity, &kontur_root, "Kontur");
  assert_eq!(events_of(&activity, "Kontur"), 0, "its runner's news stays out");

  switch_to(&desktop, EMAIL);
  let shown = || {
    record(&activity, &kontur_root, "Kontur");
    events_of(&activity, "Kontur") > 0
  };
  until(&desktop, &activity, "Kontur's events again", |_| shown());
  following.abort();
}
