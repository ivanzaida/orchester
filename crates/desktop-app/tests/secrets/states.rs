//! Empty, filtered, validation and metadata-only states.
use std::sync::atomic::Ordering;

use orchester_core::secret::Environment;

use super::harness::{Secrets, VALUE};

#[test]
fn secrets_list_filters_environments_and_shows_metadata_and_mcp_environment_uses() {
  let mut s = Secrets::new();
  let secret = s.seed("DOCS_TOKEN", Environment::Staging);
  s.seed("DOCS_TOKEN", Environment::Development);
  s.bind(&secret);
  s.list();
  assert!(s.text("secrets-list").contains("Staging"));
  assert!(s.text("secrets-list").contains("1 MCP server"));
  s.click(&format!("secret-row-{}", secret.id()));
  s.until("selected metadata", |s| {
    s.text("secret-inspector").contains("MCP · kontur-docs")
  });
  assert!(s.text("secret-inspector").contains("Never · added"));
  assert!(s.text("secret-inspector").contains("•••• ••••"));
  assert!(!s.text("app-window").contains(VALUE));
  assert_eq!(
    s.vault.reads.load(Ordering::SeqCst),
    0,
    "inspection never reads the vault"
  );
  s.choose("secrets-environment", "Production");
  assert!(s.text("secrets-empty").contains("No secrets in Production."));
  assert!(!s.exists("secret-inspector"));
  s.click("secrets-empty-add");
  assert!(
    s.text("secret-form-environment").contains("Production"),
    "add inherits the filter"
  );
}

#[test]
fn secrets_empty_project_and_usage_log_explain_what_is_available() {
  let mut s = Secrets::new();
  s.list();
  assert!(s.text("secrets-empty").contains("No secrets yet"));
  s.click("secrets-tab-usage");
  assert!(s.text("secrets-usage").contains("No access recorded yet"));
  assert!(s.text("secrets-usage").contains("Never the value"));
  s.click("secrets-tab-list");
  s.click("secrets-add");
  assert!(s.exists("secret-name"));
  assert!(s.exists("secret-value"));
  assert!(s.exists("secret-save-reason"));
  s.click("secret-save");
  assert_eq!(
    s.vault.stores.load(Ordering::SeqCst),
    0,
    "an incomplete form does nothing"
  );
  s.click("secret-cancel");
  assert!(!s.exists("secret-value"));
}

#[test]
fn secrets_name_validation_and_environment_scoped_duplicates_link_the_existing_secret() {
  let mut s = Secrets::new();
  let existing = s.seed("DOCS_TOKEN", Environment::Development);
  s.list();
  s.click("secrets-add");
  s.set("secret-value", VALUE);
  s.private();
  for name in ["bad-name".to_owned(), "A".repeat(129)] {
    s.set("secret-name", &name);
    assert!(s.exists("secret-save-reason"));
    s.click("secret-save");
    assert_eq!(s.vault.stores.load(Ordering::SeqCst), 1);
  }
  s.set("secret-name", "DOCS_TOKEN");
  assert!(s.text("secret-save-reason").contains("taken in Development"));
  assert!(s.exists("secret-open-existing"));
  s.choose("secret-form-environment", "Staging");
  assert!(!s.exists("secret-save-reason"));
  s.choose("secret-form-environment", "Development");
  s.click("secret-open-existing");
  s.until("existing inspector", |s| s.exists("secret-inspector-name"));
  assert_eq!(s.text("secret-inspector-name"), existing.name().as_str());
  assert!(!s.exists("secret-value"));
}

#[test]
fn secrets_wide_layout_uses_the_list_and_inline_editor_dimensions_and_close_clears_input() {
  let mut s = Secrets::new();
  let secret = s.seed("DOCS_TOKEN", Environment::Development);
  s.list();
  let bounds = |s: &mut Secrets, id: &str| {
    s.window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  assert_eq!(bounds(&mut s, "secret-inspector").width, 360.0);
  assert_eq!(bounds(&mut s, &format!("secret-row-{}", secret.id())).height, 48.0);
  assert_eq!(bounds(&mut s, &format!("secret-tile-{}", secret.id())).width, 28.0);
  assert_eq!(bounds(&mut s, "secret-inspector-tile").width, 32.0);
  s.click("secrets-add");
  assert_eq!(bounds(&mut s, "secret-form-close").width, 26.0);
  assert_eq!(bounds(&mut s, "secret-value-box").height, 28.0);
  assert_eq!(bounds(&mut s, "secret-project-avatar").width, 20.0);
  s.set("secret-value", VALUE);
  s.click("secret-form-close");
  assert!(!s.exists("secret-value"));
  s.click("secrets-add");
  let input = s
    .window
    .tree
    .get_element_by_id_mut("secret-value")
    .and_then(|element| element.as_text_input())
    .expect("credential input");
  assert!(input.value().is_empty(), "closing discards the credential");
}

#[test]
fn secrets_external_connection_edits_refresh_used_by_in_place() {
  let mut s = Secrets::new();
  let secret = s.seed("DOCS_TOKEN", Environment::Development);
  s.list();
  assert!(s.text("secret-uses").contains("NONE"));
  s.bind(&secret);
  s.refresh();
  s.until("updated binding", |s| {
    s.text("secret-uses").contains("MCP · kontur-docs")
  });
}

#[test]
fn secrets_loading_can_be_held_then_released() {
  use orchester_app::routes::{ProjectPage, Route};
  let mut s = Secrets::new();
  s.list();
  s.window
    .navigation()
    .go(Route::Project(super::support::project_id(), ProjectPage::Tools));
  s.window.pass();
  let hold = s.window.launch.services().expect("services").hold.clone();
  hold.set(true);
  s.window
    .navigation()
    .go(Route::Project(super::support::project_id(), ProjectPage::Secrets));
  s.until("held read", |s| s.exists("secrets-loading"));
  assert!(s.text("secrets-loading").contains("Reading the secrets"));
  hold.set(false);
  s.until("released read", |s| s.exists("secrets-empty"));
}

#[test]
fn secrets_read_failure_explains_it_and_retry_reads_the_repaired_database() {
  use orchester_app::routes::{ProjectPage, Route};
  let mut s = Secrets::new();
  s.list();
  s.window
    .navigation()
    .go(Route::Project(super::support::project_id(), ProjectPage::Tools));
  s.window.pass();
  let database = s.repo.join(".orchester/orchester.db");
  let backup = s.repo.join(".orchester/secrets-test-backup.db");
  std::fs::rename(&database, &backup).expect("move this test's database aside");
  std::fs::create_dir(&database).expect("make its path unreadable to SQLite");
  s.window
    .navigation()
    .go(Route::Project(super::support::project_id(), ProjectPage::Secrets));
  s.until("failed read", |s| s.exists("secrets-load-error"));
  assert!(s.text("secrets-load-error").contains("Secrets could not be read"));
  std::fs::remove_dir(&database).expect("remove the empty test directory");
  std::fs::rename(backup, database).expect("restore this test's database");
  s.click("secrets-load-error-retry");
  s.until("retry read", |s| s.exists("secrets-empty"));
}
