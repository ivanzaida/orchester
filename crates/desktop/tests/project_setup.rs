use std::{collections::BTreeSet, path::Path};

use orchester_core::{name::NameError, project::ProjectKeyError};
use orchester_desktop::{
  i18n::{FALLBACK_LOCALE, LOCALES},
  messages::{self, Message},
  project_setup::ProjectDraft,
};
use orchester_store::StoreError;
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
fn draft_reports_every_invalid_field() {
  let errors = ProjectDraft {
    name: "  ",
    key: "orc",
    repository: None,
  }
  .validate()
  .expect_err("nothing is valid");
  assert_eq!(errors.name.map(|message| message.key), Some("errors.nameRequired"));
  assert_eq!(errors.key.map(|message| message.key), Some("errors.projectKeyInvalid"));
  assert_eq!(
    errors.repository.map(|message| message.key),
    Some("errors.repositoryRequired")
  );
}

#[test]
fn draft_trims_the_name_and_keeps_the_repository() {
  let repo = Path::new("C:/work/kontur");
  let new = ProjectDraft {
    name: " Kontur ",
    key: "KON2",
    repository: Some(repo),
  }
  .validate()
  .expect("valid draft");
  assert_eq!(new.project.name().as_str(), "Kontur");
  assert_eq!(new.project.key().as_str(), "KON2");
  assert_eq!(new.repo_root, repo);
}

#[test]
fn draft_messages_are_translatable() {
  let messages = [
    messages::name_error(&NameError::Blank),
    messages::name_error(&NameError::TooLong { length: 201 }),
    messages::project_key_error(&ProjectKeyError::Invalid("x".into())),
    messages::repository_required(),
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
