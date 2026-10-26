//! Project settings › Sources (`kmNiF`, `lOOY6`): the empty list, a path checked in place, adding one only once a
//! person confirms it in place (window input locked meanwhile), a source runs cannot read, and removing one.
use std::path::Path;

use orchester_core::source::SourcePath;

use super::harness::Settings;

const PATH: &str = "settings-sources-add-path";
const HINT: &str = "settings-sources-add-hint-text";

fn source(path: &Path) -> SourcePath {
  SourcePath::new(&path.to_string_lossy()).expect("an absolute path")
}

/// A folder outside the project with two files.
fn docs() -> tempfile::TempDir {
  let folder = tempfile::tempdir().expect("a folder");
  for file in ["webhooks.md", "retries.md"] {
    std::fs::write(folder.path().join(file), "docs").expect("a file");
  }
  folder
}

fn checked(page: &mut Settings, path: &str, hint: &str) {
  page.set(PATH, path);
  page.until(hint, |p| p.text(HINT) == hint);
}

/// Context, with *Add a folder or file*'s path box open: the empty row's Add, or the row's own, opens it.
fn adding(page: &mut Settings) {
  page.tab("context", "settings-sources-read-row");
  if !page.exists(PATH) {
    let add = if page.exists("settings-sources-empty-add") {
      "settings-sources-empty-add"
    } else {
      "settings-sources-open-add"
    };
    page.click(add);
  }
  page.until("the path box", |p| p.exists(PATH));
}

#[test]
fn project_settings_sources_are_added_only_once_a_person_confirms_in_place() {
  let mut page = Settings::new(false);
  page.tab("context", "settings-sources-read-row");
  assert!(
    !page.exists("settings-sources-add-row-row"),
    "the empty row's Add opens it"
  );
  assert!(!page.exists("settings-sources-note"), "no count without sources");
  assert_eq!(page.text("settings-sources-empty-title"), "No sources yet");
  adding(&mut page);
  let folder = docs();
  checked(
    &mut page,
    &folder.path().to_string_lossy(),
    "Folder · 2 files · readable.",
  );
  page.click("settings-sources-add-add");
  page.until("the question", |p| p.exists("settings-sources-question-confirm"));
  assert!(page.window.link.input_gate().protecting());
  assert!(
    page
      .text("settings-sources-question")
      .contains(&*folder.path().to_string_lossy())
  );
  assert!(page.db().project_sources().expect("read").is_empty());
  // The question must live through a future pass before a person's press is taken.
  page.window.tree.tick_futures();
  page.window.pass();
  page.click("settings-sources-question-confirm");
  page.until("the stored source", |p| p.exists("settings-source-0-label"));
  assert_eq!(page.db().project_sources().expect("read"), [source(folder.path())]);
  assert_eq!(page.text("settings-sources-note"), "1 source");
  assert_eq!(page.text("settings-source-0-hint"), "Folder");
  assert!(!page.window.link.input_gate().protecting());
  page.saved();
}

#[test]
fn project_settings_sources_say_in_place_why_a_path_is_not_added() {
  let mut page = Settings::new(false);
  adding(&mut page);
  std::fs::create_dir_all(page.repo.join("docs")).expect("a project folder");
  let inside = page.repo.join("docs").to_string_lossy().into_owned();
  page.set(PATH, &inside);
  page.until("not needed", |p| {
    p.text(HINT).contains("agents already read it in the run's worktree")
  });
  let missing = docs().path().join("gone").to_string_lossy().into_owned();
  checked(
    &mut page,
    &missing,
    "Not found: there is no folder or file at this path.",
  );
  page.click("settings-sources-add-add");
  assert!(!page.exists("settings-sources-question"));
  checked(
    &mut page,
    "docs/webhooks",
    "Use a full path, from the drive or the root folder.",
  );
  assert!(page.db().project_sources().expect("read").is_empty());
}

#[test]
fn project_settings_sources_a_missing_one_says_runs_start_without_it_and_is_removed() {
  let mut page = Settings::new(false);
  let folder = docs();
  let gone = folder.path().join("runbook.md");
  page
    .db()
    .set_project_sources(&[source(folder.path()), source(&gone)])
    .expect("listed");
  page.tab("context", "settings-source-1-label");
  assert_eq!(page.text("settings-sources-note"), "2 · 1 missing");
  assert_eq!(
    page.text("settings-source-1-hint"),
    "Not found. Runs start without it; a read there asks you."
  );
  assert!(page.exists("settings-source-1-info"), "when it was checked, in its (i)");
  std::fs::write(&gone, "steps").expect("the runbook is back");
  page.click("settings-source-1-check");
  page.until("found again", |p| p.text("settings-source-1-hint") == "File");
  assert!(!page.exists("settings-source-1-check"));
  page.click("settings-source-0-remove");
  page.until("the notice", |p| p.exists("settings-sources-removed-label"));
  assert_eq!(page.db().project_sources().expect("read"), [source(&gone)]);
  assert_eq!(page.text("settings-sources-removed-hint"), "New runs start without it.");
  page.click("settings-sources-undo");
  page.until("asked again", |p| p.exists("settings-sources-question"));
  assert!(page.window.link.input_gate().protecting());
  page.window.tree.tick_futures();
  page.window.pass();
  page.click("settings-sources-question-keep");
  assert!(!page.window.link.input_gate().protecting());
  assert_eq!(page.db().project_sources().expect("read"), [source(&gone)]);
}

/// Lists a relative path among the project's sources, which no read takes: the store calls it corrupt.
fn unreadable(page: &Settings, broken: bool) {
  let sql = if broken {
    "INSERT INTO project_sources (position, path) VALUES (99, 'docs')"
  } else {
    "DELETE FROM project_sources WHERE path = 'docs'"
  };
  let changed = rusqlite::Connection::open(page.repo.join(".orchester").join("orchester.db"))
    .expect("the project database opens")
    .execute(sql, [])
    .expect("the sources change");
  assert_eq!(changed, 1);
}

#[test]
fn project_settings_sources_a_failed_read_says_why_with_try_again_never_the_list_read_before() {
  let mut page = Settings::new(false);
  let folder = docs();
  page.db().set_project_sources(&[source(folder.path())]).expect("listed");
  page.tab("context", "settings-source-0-label");
  unreadable(&page, true);
  // A change made elsewhere reads them again, which fails now.
  page.outside(orchester_workspace::tasks::outside::Outside::ProjectSettings);
  page.until("the failure", |p| p.exists("settings-sources-load-error"));
  assert!(
    page
      .text("settings-sources-load-error")
      .starts_with("The sources couldn't be read")
  );
  assert!(!page.exists("settings-source-0-label"), "the list read before is gone");
  assert!(!page.exists(PATH));
  unreadable(&page, false);
  page.click("settings-sources-load-error-retry");
  page.until("read again", |p| p.exists("settings-source-0-label"));
  assert!(!page.exists("settings-sources-load-error"));
}

/// The path's type (`uixJK` in `Q5DCn`) and the section's spacing (`oCZK1`, `VXZ0x`).
#[test]
fn project_settings_sources_theme_roles_resolve_the_design() {
  use lurq::{
    app::{App, theme::PaletteColor},
    layout::text_style::FontWeight,
    node::dimension::Dimension,
  };
  use orchester_app::theme::{self, Mode, SourcesSpacing, SourcesText};

  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  let path = theme.typography_style(SourcesText::Path.role());
  assert_eq!(
    (path.font_size, path.weight, &*path.font_family),
    (12.5, FontWeight::Normal, "Geist Mono")
  );
  assert_eq!(path.color, theme.palette_color(PaletteColor::TextPrimary));
  for (role, value) in [(SourcesSpacing::Section, 14.0), (SourcesSpacing::EmptyTop, 18.0)] {
    assert_eq!(theme.spacing_value(role.role()), Dimension::Px(value), "{role:?}");
  }
}
