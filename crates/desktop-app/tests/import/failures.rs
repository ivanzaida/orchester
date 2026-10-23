//! When Import resources cannot write: a failure writes nothing and keeps every choice, and Try again repeats the
//! same import; a copy that breaks a rule of its kind cannot be imported at all.

use orchester_app::routes::{ImportTab, ProjectPage, Route};
use orchester_core::resource::ResourceKind;

use super::harness::Import;

#[test]
fn import_that_cannot_be_written_says_why_and_tries_again() {
  let mut import = Import::new();
  import.open(ImportTab::All);
  import.go_to_create();
  // Meanwhile a newer Orchester took Kontur over: this one cannot write to it.
  let supported = import.kontur_schema(9999);
  import.click("import-submit");
  import.until("the failure", |import| import.exists("import-failed"));
  assert_eq!(
    import.text("import-failed-title"),
    "Could not import from Personal website"
  );
  assert!(!import.text("import-failed-reason").is_empty(), "the failure says why");
  assert_eq!(import.text("import-submit"), "Try again");
  assert!(import.shows("import-previous"), "the choices can still be changed");
  assert!(import.shows("import-ready-head"), "the page keeps every choice");
  import.kontur_schema(supported);
  assert_eq!(import.kontur_count(ResourceKind::Pipeline), 0, "nothing was written");
  import.click("import-submit");
  import.until("the first copy opens", |import| {
    matches!(import.route(), Route::Project(_, ProjectPage::Pipeline(..)))
  });
  let copy = import
    .kontur_resource(ResourceKind::Pipeline, "Preview release")
    .expect("the pipeline's copy");
  assert!(matches!(import.route(), Route::Project(_, ProjectPage::Pipeline(id, _)) if id == copy));
  assert_eq!(import.kontur_count(ResourceKind::Pipeline), 1, "written once");
}

#[test]
fn import_of_a_copy_that_breaks_its_kind_is_unavailable() {
  let mut import = Import::new();
  // Kontur has a Builder of its own, which Builder's copy reuses: Broken release's two step keys become one.
  let local = import.kontur_profile("builder", "Site builder");
  let broken = import.colliding_pipeline(local);
  import.open(ImportTab::All);
  import.until("the list", |import| import.exists(&format!("import-check-{broken}")));
  import.click(&format!("import-check-{broken}"));
  import.click("import-next");
  import.until("Resolve dependencies", |import| import.exists("import-mappings"));
  let builder = format!("import-status-{}", import.seeded().builder);
  assert_eq!(import.text(&builder), "Reuse local Site builder in Kontur");
  import.click("import-continue");
  import.until("Create drafts", |import| import.exists("import-ready-head"));
  assert_eq!(
    import.text("import-invalid-copy"),
    "A copy could not be prepared. The log has the details."
  );
  import.click("import-submit");
  assert!(import.shows("import-ready-head"), "Import resources stays unavailable");
  assert_eq!(import.kontur_count(ResourceKind::Pipeline), 0, "nothing was written");
}
