//! What Import resources does: its entry points open it on their kind and Back returns; selecting a pipeline takes
//! what it refers to along; a taken name blocks until the copy is renamed or left out; and nothing is written until
//! Import resources, which writes only what the person accepted and opens the first copy.

use orchester_app::routes::{ImportTab, OverviewTab, ProjectPage, Route};
use orchester_core::resource::ResourceKind;

use super::{harness::Import, support::project_id};

#[test]
fn import_opens_from_the_overview_and_back_returns_there() {
  let mut import = Import::new();
  let overview = Route::Project(project_id(), ProjectPage::Overview(OverviewTab::default()));
  import.window.navigation().go(overview.clone());
  import.until("the Overview", |import| import.exists("overview-import"));
  import.click("overview-import");
  assert_eq!(
    import.route(),
    Route::Project(project_id(), ProjectPage::Import(ImportTab::All))
  );
  import.until("the import page", |import| import.exists("screen-import_resources"));
  assert_eq!(import.text("import-crumbs"), "Kontur Import resources");
  import.click("import-back");
  assert_eq!(import.route(), overview);
}

#[test]
fn import_selecting_a_pipeline_takes_what_it_refers_to_along() {
  let mut import = Import::new();
  import.open(ImportTab::All);
  let (preview, builder) = (import.seeded().preview, import.seeded().builder);
  import.until("the list", |import| import.exists("import-list"));
  import.click(&format!("import-check-{preview}"));
  assert_eq!(
    import.text("import-footer-text"),
    "1 selected · 3 dependencies to resolve"
  );
  assert_eq!(
    import.text(&format!("import-included-{builder}")),
    "Included with Preview release"
  );
  assert!(!import.shows("import-blocked"), "Resolve dependencies is available");
  import.click(&format!("import-check-{preview}"));
  assert_eq!(import.text("import-footer-text"), "Nothing selected");
  assert!(!import.shows(&format!("import-included-{builder}")));
}

#[test]
fn import_resolve_blocks_on_a_taken_name_until_the_copy_is_renamed() {
  let mut import = Import::new();
  import.open(ImportTab::All);
  let (preview, rules, codex) = (import.seeded().preview, import.seeded().rules, import.seeded().codex);
  import.until("the list", |import| import.exists("import-list"));
  import.click(&format!("import-check-{preview}"));
  import.click("import-next");
  import.until("Resolve dependencies", |import| import.exists("import-mappings"));
  assert_eq!(import.text("import-step-resolve"), "2 Resolve dependencies");
  assert_eq!(
    import.text("import-context-text"),
    "Personal website → Kontur · Preview release v1"
  );
  assert_eq!(import.text("import-group-selected"), "SELECTED · 1");
  assert_eq!(import.text("import-group-dependencies"), "DEPENDENCIES · 3");
  assert_eq!(
    import.text(&format!("import-status-{rules}")),
    "Name taken Kontur already has Release rules"
  );
  assert_eq!(
    import.text(&format!("import-status-{codex}")),
    "Unresolved Copies show it as missing"
  );
  assert_eq!(import.text("import-blocked"), "Rename Release rules to continue.");
  import.click("import-continue");
  assert!(import.shows("import-mappings"), "Continue stays unavailable");
  assert_eq!(
    import.options(&format!("import-choice-{rules}")),
    ["New copy · Release rules", "Rename copy", "Leave out"]
  );
  import.choose(&format!("import-choice-{rules}"), "Rename copy");
  import.set(&format!("import-rename-{rules}"), "");
  assert_eq!(
    import.text(&format!("import-status-{rules}")),
    "Name needed Enter a name for the copy"
  );
  import.set(&format!("import-rename-{rules}"), "Release rules (Personal website)");
  assert_eq!(
    import.text(&format!("import-status-{rules}")),
    "Name resolved Release rules already exists"
  );
  assert_eq!(import.text("import-footer-text"), "3 new copies · 1 left unresolved");
  assert_eq!(
    import.text("import-readiness"),
    "Creates local drafts · unresolved connections, MCP servers and secrets show as missing"
  );
  import.click("import-previous");
  import.until("Select resources", |import| import.exists("import-list"));
  assert_eq!(
    import.text("import-footer-text"),
    "1 selected · 3 dependencies to resolve",
    "Back keeps the selection"
  );
}

#[test]
fn import_writes_only_what_the_person_accepted_and_opens_the_first_copy() {
  let mut import = Import::new();
  import.open(ImportTab::All);
  import.go_to_create();
  assert_eq!(import.text("import-step-create"), "3 Create drafts");
  assert_eq!(import.text("import-created-eyebrow"), "CREATES · 3 LOCAL DRAFTS");
  assert_eq!(import.text("import-created-0-name"), "Preview release");
  assert_eq!(
    import.text("import-created-0-from"),
    "from Personal website · Preview release v1"
  );
  assert_eq!(import.text("import-created-0-kind"), "Draft pipeline v1");
  assert_eq!(
    import.text("import-created-2-from"),
    "from Personal website · Release rules draft · renamed"
  );
  assert_eq!(import.text("import-missing-eyebrow"), "LEFT UNRESOLVED · 1");
  assert_eq!(import.text("import-authorized-meta"), "Nothing");
  assert_eq!(
    import.kontur_count(ResourceKind::Pipeline),
    0,
    "nothing is written before Import resources"
  );
  import.click("import-submit");
  import.until("the first copy opens", |import| {
    matches!(import.route(), Route::Project(_, ProjectPage::Pipeline(..)))
  });
  let copy = import
    .kontur_resource(ResourceKind::Pipeline, "Preview release")
    .expect("the pipeline's copy");
  assert!(matches!(import.route(), Route::Project(_, ProjectPage::Pipeline(id, _)) if id == copy));
  assert_eq!(import.kontur_count(ResourceKind::Pipeline), 1);
  assert_eq!(
    import.kontur_count(ResourceKind::AgentProfile),
    1,
    "Builder, not Reviewer"
  );
  assert!(import.kontur_resource(ResourceKind::AgentProfile, "Builder").is_some());
  assert!(
    import
      .kontur_resource(ResourceKind::Instruction, "Release rules (Personal website)")
      .is_some()
  );
  assert_eq!(import.kontur_count(ResourceKind::Instruction), 2);
}

#[test]
fn import_leaves_out_what_the_person_leaves_out() {
  let mut import = Import::new();
  import.open(ImportTab::All);
  let (preview, rules) = (import.seeded().preview, import.seeded().rules);
  import.until("the list", |import| import.exists("import-list"));
  import.click(&format!("import-check-{preview}"));
  import.click("import-next");
  import.until("Resolve dependencies", |import| import.exists("import-mappings"));
  import.choose(&format!("import-choice-{rules}"), "Leave out");
  assert_eq!(import.text(&format!("import-status-{rules}")), "Left out Not imported");
  import.click("import-continue");
  import.until("Create drafts", |import| import.exists("import-ready-head"));
  assert_eq!(import.text("import-created-eyebrow"), "CREATES · 2 LOCAL DRAFTS");
  import.click("import-submit");
  import.until("the first copy opens", |import| {
    matches!(import.route(), Route::Project(_, ProjectPage::Pipeline(..)))
  });
  assert_eq!(
    import.kontur_count(ResourceKind::Instruction),
    1,
    "only Kontur's own rules"
  );
}

#[test]
fn import_that_finds_this_project_changed_writes_nothing_and_checks_the_mappings_again() {
  let mut import = Import::new();
  import.open(ImportTab::All);
  let builder = import.seeded().builder;
  import.go_to_create();
  // Meanwhile Kontur got an agent profile with the slug Builder's copy was to take.
  import.kontur_profile("builder", "Site builder");
  import.click("import-submit");
  import.until("the failure", |import| import.exists("import-failed"));
  assert_eq!(
    import.text("import-failed-title"),
    "This project changed while you chose"
  );
  assert_eq!(import.kontur_count(ResourceKind::Pipeline), 0, "nothing was written");
  assert_eq!(import.kontur_count(ResourceKind::AgentProfile), 1);
  assert!(
    !import.shows("import-previous"),
    "the failure asks to check the mappings"
  );
  import.click("import-check");
  // Both projects are read again; the row shows what it showed until then, then Kontur's own profile.
  let status = format!("import-status-{builder}");
  import.until("Builder reuses Kontur's profile", |import| {
    import.exists(&status) && import.text(&status) == "Reuse local Site builder in Kontur"
  });
  assert!(import.shows("import-mappings"), "back on Resolve dependencies");
  assert!(!import.shows("import-failed"));
}
