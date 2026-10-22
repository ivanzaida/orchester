//! What Select resources shows: the projects, the kind tabs with their counts and the source's resources; and in
//! place of the list, why there is none: no other project, a source that cannot be read, a search that matches
//! nothing, and the Secrets tab.

use orchester_app::routes::{ImportTab, ProjectPage, Route};

use super::{harness::Import, support::project_id};

#[test]
fn import_select_lists_the_source_with_its_counts() {
  let mut import = Import::new();
  import.open(ImportTab::All);
  let (preview, builder) = (import.seeded().preview, import.seeded().builder);
  import.until("the list", |import| import.exists("import-list"));
  assert_eq!(import.text("import-title"), "Import resources");
  assert_eq!(import.text("import-step-select"), "1 Select resources");
  assert_eq!(import.text("import-step-create"), "3 Create drafts");
  assert_eq!(import.options("import-source"), ["Personal website"]);
  assert_eq!(import.text("import-destination"), "Kontur · independent project copies");
  assert_eq!(import.text("import-tab-all"), "All 4");
  assert_eq!(import.text("import-tab-agents"), "Agents 2");
  assert_eq!(
    import.text("import-secrets-reason"),
    "Secrets are not imported: a secret's value stays in its own project."
  );
  assert_eq!(
    import.text(&format!("import-summary-{preview}")),
    "Pipeline · 1 step · 3 dependencies"
  );
  assert_eq!(
    import.text(&format!("import-depends-{preview}")),
    "Builder, Release rules +1"
  );
  assert_eq!(
    import.text(&format!("import-summary-{builder}")),
    "Agent profile · Used by Preview release"
  );
  assert_eq!(import.text(&format!("import-version-{preview}")), "Published v1");
  assert_eq!(import.text("import-footer-text"), "Nothing selected");
  assert_eq!(import.text("import-blocked"), "Select a resource to continue.");
  import.click("import-next");
  assert!(import.shows("import-list"), "Resolve dependencies stays unavailable");
}

#[test]
fn import_select_tab_lists_its_kind_only() {
  let mut import = Import::new();
  import.open(ImportTab::Agents);
  let seeded = (
    import.seeded().builder,
    import.seeded().reviewer,
    import.seeded().preview,
  );
  import.until("the list", |import| import.exists("import-list"));
  assert!(import.shows(&format!("import-resource-{}", seeded.0)));
  assert!(import.shows(&format!("import-resource-{}", seeded.1)));
  assert!(!import.shows(&format!("import-resource-{}", seeded.2)));
  import.click("import-tab-tools");
  assert_eq!(
    import.route(),
    Route::Project(project_id(), ProjectPage::Import(ImportTab::Tools)),
    "a tab is a place of its own"
  );
  import.until("the empty kind", |import| import.exists("import-no-match"));
  assert_eq!(import.text("import-no-match-title"), "Nothing of this kind");
}

#[test]
fn import_select_search_that_matches_nothing_offers_to_clear_it() {
  let mut import = Import::new();
  import.open(ImportTab::All);
  import.until("the list", |import| import.exists("import-list"));
  import.set("import-search", "staging deploy");
  assert_eq!(import.text("import-no-match-title"), "No resources match.");
  assert_eq!(
    import.text("import-no-match-text"),
    "Nothing in Personal website is called staging deploy. Clear the search or pick another kind."
  );
  import.click("import-clear-search");
  assert!(import.shows("import-list"));
  import.set("import-search", "  REVIEW");
  let reviewer = import.seeded().reviewer;
  assert!(import.shows(&format!("import-resource-{reviewer}")));
  assert!(!import.shows(&format!("import-resource-{}", import.seeded().builder)));
}

#[test]
fn import_secrets_tab_says_why_and_shows_everything_else() {
  let mut import = Import::new();
  import.open(ImportTab::Secrets);
  import.until("the secrets state", |import| import.exists("import-secrets"));
  assert_eq!(import.text("import-secrets-title"), "Secrets are not imported yet");
  import.click("import-show-all");
  assert_eq!(
    import.route(),
    Route::Project(project_id(), ProjectPage::Import(ImportTab::All))
  );
  import.until("the list", |import| import.exists("import-list"));
}

#[test]
fn import_without_another_project_says_there_is_none() {
  let mut import = Import::alone();
  import.open(ImportTab::All);
  import.until("the empty state", |import| import.exists("import-no-sources"));
  assert_eq!(import.text("import-no-sources-title"), "No other projects");
  assert_eq!(
    import.text("import-blocked"),
    "There is no other project to import from."
  );
  assert!(!import.shows("import-tabs"), "no kinds to tell apart");
  import.click("import-all-projects");
  assert_eq!(import.route(), Route::AllProjects);
}

#[test]
fn import_source_that_cannot_be_read_says_why_and_tries_again() {
  let mut import = Import::new();
  let source = import.source.as_ref().expect("Personal website").0.clone();
  // A newer Orchester wrote Personal website: this one does not read it.
  rusqlite::Connection::open(source.join(".orchester").join("orchester.db"))
    .and_then(|db| db.pragma_update(None, "user_version", 9999))
    .expect("Personal website's schema version");
  import.open(ImportTab::All);
  import.until("the notice", |import| import.exists("import-unreadable"));
  assert_eq!(
    import.text("import-unreadable-title"),
    "Personal website was written by a newer version of Orchester."
  );
  assert_eq!(
    import.text("import-unreadable-body"),
    "Choose another project, or try again once Personal website opens on its own."
  );
  assert_eq!(import.text("import-blocked"), "Choose a project to import from.");
  assert!(!import.shows("import-tabs"));
  import.click("import-retry");
  import.until("the notice again", |import| import.exists("import-unreadable"));
}
