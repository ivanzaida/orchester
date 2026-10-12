use std::fs;

use orchester_core::{
  id::SecretId,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::SecretStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  domain_support::Fixture,
  integration_support::{PRIVATE_VALUE, Page, contains},
};

#[test]
fn local_integration_known_secret_filename_never_reaches_actual_window_tree_ids_or_display() {
  let fixture = Fixture::new(false);
  let name = format!("notes-{PRIVATE_VALUE}.txt");
  fs::write(fixture.root().join(&name), b"ordinary private fixture bytes").expect("private filename");
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  db.create_secret(&Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("PRIVATE_FIXTURE").expect("name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  ))
  .expect("private secret metadata");
  let mut page = Page::new(fixture);
  page.ready("local-read-error");
  let scope = page
    .window
    .tree
    .get_element_by_id("app-window")
    .expect("actual window including every local control");
  assert!(
    !contains(scope, PRIVATE_VALUE),
    "display, IDs and text cannot expose the known private filename"
  );
  assert!(!page.has("local-files"));
  assert!(
    page
      .window
      .texts("local-read-error")
      .expect("visible refusal")
      .join(" ")
      .contains("Sensitive checkout metadata")
  );
  assert_eq!(
    fs::read(page.root().join(&name)).expect("original remains"),
    b"ordinary private fixture bytes"
  );
  assert_eq!(name, format!("notes-{PRIVATE_VALUE}.txt"));
}

#[test]
fn local_integration_known_secret_root_is_hidden_from_shell_caption_tooltip_and_recent_options() {
  let fixture = Fixture::named(false, PRIVATE_VALUE);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  db.create_secret(&Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("PRIVATE_FIXTURE").expect("name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  ))
  .expect("private metadata");
  let mut page = Page::new(fixture);
  page.ready("local-read-error");
  let window = page.window.tree.get_element_by_id("app-window").expect("actual window");
  assert!(
    !contains(window, PRIVATE_VALUE),
    "the local-integration shell cannot expose its known-secret root"
  );
  page.click("project-switcher");
  page.window.until("private recent option projection", |window| {
    window
      .texts("project-switcher-menu")
      .is_some_and(|texts| texts.join(" ").contains("Sensitive metadata hidden"))
  });
  assert!(!contains(
    page.window.tree.get_element_by_id("app-window").expect("actual tree"),
    PRIVATE_VALUE
  ));
}
