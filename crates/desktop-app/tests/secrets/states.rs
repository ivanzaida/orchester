//! Project settings › Secrets (`06 Secrets` `bVh1e`): its groups by environment and the filter, a secret's page
//! (`06b` `famwr`), nothing yet (`08` `aalCu`), New secret's validation, and reading and its failure.
use std::sync::atomic::Ordering;

use orchester_core::secret::Environment;

use super::harness::{Secrets, VALUE};

/// The reason New secret's Save cannot be used, under its actions.
const REASON: &str = "secret-form-actions-reason";

#[test]
fn secrets_list_groups_by_environment_and_a_secrets_page_shows_metadata_and_mcp_uses() {
  let mut s = Secrets::new();
  let secret = s.seed("DOCS_TOKEN", Environment::Staging);
  s.seed("DOCS_TOKEN", Environment::Development);
  s.bind(&secret);
  s.list();
  assert!(s.text("secrets-group-staging").contains("Staging"));
  assert!(s.text("secrets-group-development").contains("Development"));
  assert!(
    s.text(&format!("secret-row-{}-row", secret.id()))
      .contains("kontur-docs")
  );
  // The tab's save status is its variables'; the secrets save per action.
  s.until("the variables' status", |s| s.exists("settings-variable-count"));
  assert_eq!(s.text("project-settings-save-status"), "All changes saved");
  s.click(&format!("secret-row-{}-row", secret.id()));
  s.until("its page", |s| {
    s.exists("secret-uses") && s.text("secret-uses").contains("kontur-docs")
  });
  assert_eq!(s.text("secret-crumb-back"), "Environment");
  assert!(!s.exists("settings-variable-count"), "a secret's page has no variables");
  assert!(s.text("secret-inspector").contains("Never · added today"));
  // *Details* as `06b` draws them: no Kind or Value row; *Used by* names the variable the server is given it in.
  assert!(!s.exists("secret-kind-value") && !s.exists("secret-value"));
  assert!(
    s.text("secret-uses").contains("MCP server · variable TOKEN"),
    "{}",
    s.text("secret-uses")
  );
  assert!(!s.text("app-window").contains(VALUE));
  assert_eq!(
    s.vault.reads.load(Ordering::SeqCst),
    0,
    "inspection never reads the vault"
  );
  s.click("secret-crumb-back");
  s.until("the list", |s| s.exists("secrets-filter"));
  s.click("secrets-environment-production");
  s.until("the filtered list", |s| s.exists("secrets-environment-empty"));
  assert!(
    s.text("secrets-environment-empty").contains("Production"),
    "{}",
    s.text("secrets-environment-empty")
  );
  assert!(!s.exists("secrets-group-staging"), "only Production's group");
  s.click("secrets-add");
  s.until("New secret", |s| s.exists("secret-form-environment"));
  assert!(
    s.text("secret-form-environment").contains("Production"),
    "add inherits the filter"
  );
}

/// A project with no secret says so in one quiet row with Add secret and Import, and Recent access says it is not
/// recorded yet.
#[test]
fn secrets_empty_project_offers_add_and_import() {
  let mut s = Secrets::new();
  s.list();
  assert!(s.text("secrets-empty").contains("No secrets yet"));
  assert!(s.exists("secrets-empty-add"));
  assert!(s.exists("secrets-empty-import"));
  s.click("secrets-empty-add");
  s.until("New secret", |s| s.exists("secret-value"));
  assert!(s.exists("secret-name"));
  assert!(s.exists(REASON));
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
fn secrets_recent_access_says_it_is_not_recorded_yet() {
  let mut s = Secrets::new();
  let secret = s.seed("DOCS_TOKEN", Environment::Development);
  s.list();
  assert!(
    s.text(&format!("secret-mask-{}", secret.id())).ends_with("set today"),
    "a value set today says so, without its date"
  );
  s.click(&format!("secret-row-{}-row", secret.id()));
  s.until("its page", |s| s.exists("secret-recent"));
  // Unavailable, with why, rather than an empty group.
  assert!(
    s.text("secret-recent-unavailable")
      .contains("Access to secrets is recorded once tools can use them")
  );
  s.click("secret-more");
  assert!(s.exists("secret-copy-name"), "the … menu holds Copy name");
}

/// An archived secret stays in its environment's group (`OLD_FIGMA_TOKEN` in `06`), counted, and says so.
#[test]
fn secrets_list_shows_archived_secrets_in_their_group() {
  let mut s = Secrets::new();
  s.seed("DOCS_TOKEN", Environment::Development);
  let old = s.seed("OLD_TOKEN", Environment::Development);
  orchester_workspace::secrets::data::archive_in(&s.repo, s.vault.as_ref(), old.id()).expect("archive");
  s.list();
  let row = format!("secret-row-{}-row", old.id());
  s.until("the archived row", |s| s.exists(&row));
  assert!(
    s.text(&row).contains("Archived · no tool or MCP server uses it"),
    "{}",
    s.text(&row)
  );
  assert!(!s.exists(&format!("secret-mask-{}", old.id())), "no value to show");
  assert!(s.text("secrets-group-development").contains("2 secrets"));
  assert!(s.text("secrets-handling").contains("never shown again"));
}

#[test]
fn secrets_name_validation_and_environment_scoped_duplicates_link_the_existing_secret() {
  let mut s = Secrets::new();
  let existing = s.seed("DOCS_TOKEN", Environment::Development);
  s.list();
  s.click("secrets-add");
  s.until("New secret", |s| s.exists("secret-value"));
  s.set("secret-value", VALUE);
  s.private();
  for name in ["bad-name".to_owned(), "A".repeat(129)] {
    s.set("secret-name", &name);
    assert!(s.exists(REASON));
    s.click("secret-save");
    assert_eq!(s.vault.stores.load(Ordering::SeqCst), 1);
  }
  s.set("secret-name", "DOCS_TOKEN");
  assert!(s.text(REASON).contains("taken in Development"));
  assert!(s.exists("secret-open-existing"));
  s.choose("secret-form-environment", "Staging");
  assert!(!s.exists(REASON));
  s.choose("secret-form-environment", "Development");
  s.click("secret-open-existing");
  s.until("the existing secret's page", |s| s.exists("secret-inspector-name"));
  assert_eq!(s.text("secret-inspector-name"), existing.name().as_str());
  assert!(!s.exists("secret-value-field"));
}

/// The groups keep the page's 720 column, and how values are kept is said under them.
#[test]
fn secrets_groups_keep_the_column_with_the_handling_note_under_them() {
  let mut s = Secrets::new();
  let entries: Vec<_> = (0..5)
    .map(|index| s.seed(&format!("TOKEN_{index}"), Environment::Development))
    .collect();
  s.list();
  let bounds = |s: &mut Secrets, id: &str| {
    s.window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let group = bounds(&mut s, "secrets-group-development");
  assert!(group.width <= 720.5, "{group:?}");
  for secret in entries {
    let row = bounds(&mut s, &format!("secret-row-{}-row", secret.id()));
    assert!(
      row.y + row.height <= group.y + group.height + 0.5,
      "every row is in its group: {row:?} in {group:?}"
    );
  }
  let notice = bounds(&mut s, "secrets-handling");
  assert!(notice.y >= group.y + group.height, "the note follows the groups");
}

#[test]
fn secrets_external_connection_edits_refresh_used_by_in_place() {
  let mut s = Secrets::new();
  let secret = s.seed("DOCS_TOKEN", Environment::Development);
  s.list();
  s.click(&format!("secret-row-{}-row", secret.id()));
  s.until("its page", |s| s.exists("secret-use-none"));
  s.bind(&secret);
  s.refresh();
  s.until("updated binding", |s| {
    s.exists("secret-uses") && s.text("secret-uses").contains("kontur-docs")
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
  s.window.navigation().go(Route::Project(
    super::support::project_id(),
    ProjectPage::Secrets(Default::default()),
  ));
  s.until("held read", |s| s.exists("secrets-loading"));
  // While the secrets are read the toolbar works over their bones (`12 · Project settings · Secrets · loading`).
  assert!(s.exists("secrets-add"), "Add secret works while the secrets are read");
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
  // Tools reads this database asynchronously; its empty state means the read has released its handle.
  s.until("Tools read before moving the database", |s| s.exists("tools-empty"));
  let database = s.repo.join(".orchester/orchester.db");
  let backup = s.repo.join(".orchester/secrets-test-backup.db");
  // Other project feeds can finish after Tools; Windows refuses the rename until their handles close.
  s.until("database readers release the fixture", |_| {
    match std::fs::rename(&database, &backup) {
      Ok(()) => true,
      Err(error) if cfg!(windows) && error.raw_os_error() == Some(32) => false,
      Err(error) => panic!("move this test's database aside: {error}"),
    }
  });
  std::fs::create_dir(&database).expect("make its path unreadable to SQLite");
  s.window.navigation().go(Route::Project(
    super::support::project_id(),
    ProjectPage::Secrets(Default::default()),
  ));
  s.until("failed read", |s| s.exists("secrets-load-error"));
  assert!(s.text("secrets-load-error").contains("Secrets could not be read"));
  std::fs::remove_dir(&database).expect("remove the empty test directory");
  std::fs::rename(backup, database).expect("restore this test's database");
  s.click("secrets-load-error-retry");
  s.until("retry read", |s| s.exists("secrets-empty"));
}
