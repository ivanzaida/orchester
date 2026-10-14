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
  assert_eq!(
    readiness(true, None),
    Readiness::Ready,
    "a detection still running never holds Create back"
  );
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
    assert_eq!(
      readiness(true, Some(&state)),
      Readiness::Ready,
      "{state:?} is GitHub's, optional"
    );
  }
}

#[test]
fn setup_uses_a_detection_only_for_its_folder() {
  use orchester_core::repository::{Detection, RemoteName};
  use orchester_workspace::project_setup::{FolderDetection, expected_account};
  let local = Detection::NoRemote(RemoteName::origin());
  let (here, there) = (Path::new("C:/work/kontur"), Path::new("C:/work/atlas"));
  assert_eq!(expected_account(None, here, &local), None, "no GitHub, no account");
  let detected = FolderDetection::new(here.to_path_buf(), local.clone());
  assert_eq!(detected.of(Some(here)), Some(&local));
  assert_eq!(
    detected.of(Some(there)),
    None,
    "a detection is never used for another folder"
  );
  assert_eq!(detected.of(None), None);
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
    link: None,
  };
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let activity = Activity::new();
  let created = runtime
    .block_on(create(&workspace, &activity, creation("Kontur")))
    .expect("created");
  assert_eq!(created.project.project.key().as_str(), "KON");
  assert_eq!(workspace.projects().expect("the list").len(), 1);
  // Removed from the list, its data still in the folder.
  workspace.forget(created.project.project.id()).expect("forgotten");
  let failure = runtime
    .block_on(create(&workspace, &activity, creation("Kontur")))
    .expect_err("one project per repository");
  assert_eq!(failure.place, ErrorPlace::Repository);
  assert_eq!(failure.message.key, "errors.alreadyInitialized");
}

#[test]
fn setup_refuses_a_folder_the_list_holds_before_writing_anything() {
  use orchester_store::AppDb;
  use orchester_workspace::{
    activity::Activity,
    project_setup::{CreateCause, Creation, create},
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
    link: None,
  };
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let activity = Activity::new();
  runtime
    .block_on(create(&workspace, &activity, creation("Kontur")))
    .expect("created");
  // The project's data is gone, but the list still holds the project.
  std::fs::remove_dir_all(repo.join(".orchester")).expect("the data is gone");
  let failure = runtime
    .block_on(create(&workspace, &activity, creation("Kontur 2")))
    .expect_err("the list holds the folder");
  assert_eq!(failure.cause, CreateCause::AlreadyListed);
  assert_eq!(failure.message.key, "errors.alreadyListed");
  assert_translatable(&failure.message);
  assert!(!repo.join(".orchester").exists(), "nothing is written");
}

/// `gh` and git answering for a repository on github.com whose `gh` acts as `login`.
struct SignedIn(&'static str);

impl orchester_core::command::CommandRunner for SignedIn {
  fn run(
    &self,
    command: &orchester_core::command::Command,
  ) -> Result<orchester_core::command::CommandOutput, orchester_core::command::CommandError> {
    let args = command.args().join(" ");
    let stdout = if args.ends_with("remote -v") {
      "origin	https://github.com/ivanzaida/kontur.git (fetch)
origin	https://github.com/ivanzaida/kontur.git (push)
"
      .to_owned()
    } else if args == "--version" {
      "gh version 2.62.0 (2024-11-14)
"
      .to_owned()
    } else if args.starts_with("auth status") {
      format!(
        r#"{{"hosts":{{"github.com":[{{"state":"success","active":true,"host":"github.com","login":"{}","scopes":"repo","gitProtocol":"https"}}]}}}}"#,
        self.0
      )
    } else {
      r#"{"nameWithOwner":"ivanzaida/kontur","defaultBranchRef":{"name":"main"},"viewerPermission":"ADMIN"}"#.to_owned()
    };
    Ok(orchester_core::command::CommandOutput {
      code: Some(0),
      stdout,
      stderr: String::new(),
    })
  }
}

fn github_detection(login: &'static str) -> orchester_core::repository::Detection {
  use orchester_core::{
    connection::GitHubHost,
    repository::{DetectRequest, GhProgram, RemoteName, detect_repository},
    time::Timestamp,
  };
  let programs = [GhProgram {
    host: GitHubHost::github_com(),
    executable: Some("gh".into()),
  }];
  let request = DetectRequest {
    repo_root: Path::new("."),
    git: Path::new("git"),
    remote: &RemoteName::origin(),
    programs: &programs,
  };
  detect_repository(&SignedIn(login), &request, Timestamp::from_unix_millis(0))
}

#[test]
fn setup_expects_the_account_only_for_the_folder_and_host_it_was_taken_for() {
  use orchester_workspace::project_setup::{ExpectedAccount, expected_account};
  let (here, there) = (Path::new("C:/work/kontur"), Path::new("C:/work/atlas"));
  let first = github_detection("ivanzaida");
  let pinned = expected_account(None, here, &first).expect("gh acts as an account");
  assert_eq!(pinned.login().as_str(), "ivanzaida");
  let switched = github_detection("ivan-work");
  assert_eq!(
    expected_account(Some(&pinned), here, &switched).map(|expected| expected.login().clone()),
    Some(pinned.login().clone()),
    "Check again in the same folder keeps what the project expects"
  );
  assert_eq!(
    expected_account(Some(&pinned), there, &switched).map(|expected| expected.login().as_str().to_owned()),
    Some("ivan-work".to_owned()),
    "another folder starts from the account gh acts as there"
  );
  let local = orchester_core::repository::Detection::NoRemote(orchester_core::repository::RemoteName::origin());
  assert!(
    !pinned.applies_to(here, &local),
    "a folder without GitHub expects nobody"
  );
  let used = ExpectedAccount::acting(here, &switched).expect("Use this account");
  assert_eq!(used.login().as_str(), "ivan-work");
}
