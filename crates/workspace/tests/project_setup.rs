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
    StoreError::Unrepresentable("repository path"),
    StoreError::EmptyBatch,
  ];
  for error in &errors {
    assert_translatable(&messages::store_error(error));
  }
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
