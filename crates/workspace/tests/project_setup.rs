use std::{collections::BTreeSet, path::Path};

use orchester_core::name::NameError;
use orchester_store::StoreError;
use orchester_workspace::{
  display,
  i18n::{FALLBACK_LOCALE, LOCALES},
  messages::{self, Message},
  project_setup::{CreateFailure, ErrorPlace, ProjectDraft},
};
use serde_json::Value;

fn locale() -> Value {
  let (_, contents) = LOCALES
    .iter()
    .find(|(locale, _)| *locale == FALLBACK_LOCALE)
    .expect("the fallback locale ships");
  serde_json::from_str(contents).expect("locale file is valid JSON")
}

/// The message's key resolves to a string whose placeholders are exactly the message's args.
fn assert_translatable(message: &Message) {
  let locale = locale();
  let text = message
    .key
    .split('.')
    .try_fold(&locale, |value, part| value.get(part))
    .and_then(Value::as_str)
    .unwrap_or_else(|| panic!("`{}` is missing from {FALLBACK_LOCALE}.json", message.key));
  let placeholders: BTreeSet<&str> = text
    .split("{{")
    .skip(1)
    .filter_map(|part| part.split_once("}}").map(|(name, _)| name.trim()))
    .collect();
  let args: BTreeSet<&str> = message.args.iter().map(|(name, _)| *name).collect();
  assert_eq!(placeholders, args, "placeholders of `{}`", message.key);
}

#[test]
fn draft_reports_a_blank_name_and_a_missing_repository() {
  let errors = ProjectDraft {
    name: "  ",
    repository: None,
  }
  .validate()
  .expect_err("nothing is valid");
  assert_eq!(errors.name.map(|message| message.key), Some("errors.nameRequired"));
  assert!(errors.repository_missing);
}

#[test]
fn draft_trims_the_name_and_suggests_the_key() {
  let repo = Path::new("C:/work/kontur");
  let new = ProjectDraft {
    name: " Kontur ",
    repository: Some(repo),
  }
  .validate()
  .expect("valid draft");
  assert_eq!(new.project.name().as_str(), "Kontur");
  assert_eq!(new.project.key().as_str(), "KON");
  assert_eq!(new.repo_root, repo);
}

#[test]
fn long_paths_keep_the_repository_folder() {
  let path = Path::new(r"H:\projects\orchester\.tmp\worktrees\claude-desktop-onboarding\.tmp\ui\kontur");
  let short = display::shorten_path(path, 40);
  assert!(short.chars().count() <= 40, "{short}");
  assert!(short.starts_with(r"H:\…\"), "{short}");
  assert!(short.ends_with(r"\ui\kontur"), "{short}");
  assert_eq!(
    display::shorten_path(Path::new(r"C:\work\kontur"), 40),
    r"C:\work\kontur"
  );
}

#[test]
fn draft_messages_are_translatable() {
  let messages = [
    messages::name_error(&NameError::Blank),
    messages::name_error(&NameError::TooLong { length: 201 }),
    messages::interrupted(),
  ];
  messages.iter().for_each(assert_translatable);
}

#[test]
fn store_error_messages_are_translatable() {
  let path = || Path::new("C:/work/kontur").to_path_buf();
  let errors = [
    StoreError::NotAGitRepository(path()),
    StoreError::AlreadyInitialized(path()),
    StoreError::NotInitialized(path()),
    StoreError::SchemaTooNew { found: 9, supported: 1 },
    StoreError::UnrepresentablePath("repository path"),
    StoreError::Unrepresentable("task time limit"),
    StoreError::EmptyBatch,
  ];
  for error in &errors {
    assert_translatable(&messages::store_error(error));
  }
  // A number too large to store is not a folder path.
  assert_eq!(
    messages::store_error(&StoreError::Unrepresentable("task time limit")).key,
    "errors.unstorableValue"
  );
  assert_eq!(
    messages::store_error(&StoreError::UnrepresentablePath("repository path")).key,
    "errors.unsupportedPath"
  );
}

#[test]
fn creation_failure_is_retired_when_name_or_folder_change() {
  let notgit = Path::new("C:/work/notgit");
  let repo = Path::new("C:/work/kontur");
  let new = ProjectDraft {
    name: "Kontur",
    repository: Some(notgit),
  }
  .validate()
  .expect("valid draft");
  let failure = CreateFailure::from_store(&new, &StoreError::NotAGitRepository(notgit.to_path_buf()));
  assert_eq!(failure.place, ErrorPlace::Repository);
  let draft = |name, repository| ProjectDraft { name, repository };
  assert!(failure.applies_to(&draft(" Kontur ", Some(notgit))));
  assert!(!failure.applies_to(&draft("Kontur", Some(repo))));
  assert!(!failure.applies_to(&draft("Kontur 2", Some(notgit))));
  assert!(!failure.applies_to(&draft("Kontur", None)));
}

#[test]
fn creation_failures_not_about_the_folder_stay_beside_create() {
  let new = ProjectDraft {
    name: "Kontur",
    repository: Some(Path::new("C:/work/kontur")),
  }
  .validate()
  .expect("valid draft");
  let failure = CreateFailure::from_store(&new, &StoreError::EmptyBatch);
  assert_eq!(failure.place, ErrorPlace::Form);
}

#[test]
fn setup_readiness_needs_a_folder_that_is_a_repository() {
  use orchester_core::{connection::GitHubLogin, repository::DetectionState};
  use orchester_workspace::project_setup::{Readiness, readiness};
  assert_eq!(readiness(false, None), Readiness::NoFolder);
  assert_eq!(readiness(true, None), Readiness::Ready, "a detection still running never holds Create back");
  assert_eq!(
    readiness(true, Some(&DetectionState::NotAGitRepository)),
    Readiness::NotARepository
  );
  let account = GitHubLogin::new("ivanzaida").expect("a login");
  for state in [
    DetectionState::NoRemote,
    DetectionState::GhNotFound,
    DetectionState::NotSignedIn,
    DetectionState::NoAccess { account },
  ] {
    assert_eq!(readiness(true, Some(&state)), Readiness::Ready, "{state:?} is GitHub's, optional");
  }
}

#[test]
fn setup_keeps_the_account_a_project_expects() {
  use orchester_core::{
    connection::GitHubLogin,
    repository::{Detection, RemoteName},
  };
  use orchester_workspace::project_setup::expected_account;
  let local = Detection::NoRemote(RemoteName::origin());
  assert_eq!(expected_account(None, &local), None);
  let expected = GitHubLogin::new("ivanzaida").expect("a login");
  assert_eq!(expected_account(Some(&expected), &local), Some(expected));
}

#[test]
fn setup_creates_the_project_and_reports_a_second_one_in_the_folder() {
  use orchester_store::AppDb;
  use orchester_workspace::{
    activity::Activity,
    project_setup::{Creation, create},
    workspace::Workspace,
  };
  let dir = tempfile::tempdir().expect("a folder");
  let repo = dir.path().join("kontur");
  std::fs::create_dir_all(repo.join(".git")).expect("a repository");
  let workspace = Workspace::new(AppDb::open(&dir.path().join("orchester.db")).expect("a database"));
  let creation = |name: &str| Creation {
    new: ProjectDraft {
      name,
      repository: Some(&repo),
    }
    .validate()
    .expect("a valid draft"),
    detection: None,
    expected: None,
    connection_name: "GitHub".to_owned(),
  };
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let activity = Activity::new();
  let created = runtime
    .block_on(create(&workspace, &activity, creation("Kontur")))
    .expect("created");
  assert_eq!(created.project.key().as_str(), "KON");
  assert_eq!(workspace.projects().expect("the list").len(), 1);
  let failure = runtime
    .block_on(create(&workspace, &activity, creation("Kontur")))
    .expect_err("one project per repository");
  assert_eq!(failure.place, ErrorPlace::Repository);
  assert_eq!(failure.message.key, "errors.alreadyInitialized");
}
