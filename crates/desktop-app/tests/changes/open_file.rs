//! *Open file* on a changed file's head opens the file in the code viewer in a new tab: a checkpointed change
//! at the checkpoint, a change not checkpointed yet in the run's worktree.

use orchester_app::routes::{ChangeSource, ProjectPage, Route};
use orchester_workspace::files::Version;

use super::Page;

/// The code viewer's file and version the active tab shows.
fn shown(page: &Page) -> (String, Version) {
  match page.window.navigation().route_now() {
    Route::Project(_, ProjectPage::Files(view)) => (view.file().expect("a file").to_owned(), view.at().clone()),
    other => panic!("not the code viewer: {other:?}"),
  }
}

/// Waits until the Changes tab shows the diff of `path`.
fn showing(page: &mut Page, path: &str) {
  page.window.until(path, |window| {
    window.texts("changes-diff-path").is_some_and(|texts| texts == [path])
      && window.tree.get_element_by_id("changes-open-file").is_some()
  });
}

#[test]
fn changes_open_file_opens_the_file_in_a_new_tab_at_the_version_of_its_change() {
  let mut page = Page::open();
  page.open_file("KON-12", "src/webhooks/log.rs", ChangeSource::Worktree);
  showing(&mut page, "src/webhooks/log.rs");
  let tabs = page.window.navigation().tabs_now().tabs().len();
  page.click("changes-open-file");
  assert_eq!(page.window.navigation().tabs_now().tabs().len(), tabs + 1, "a new tab");
  let (file, at) = shown(&page);
  assert_eq!(file, "src/webhooks/log.rs");
  assert!(matches!(at, Version::Run(_)), "the run's worktree: {at:?}");
  page.window.until("the file", |window| {
    window.tree.get_element_by_id("files-code").is_some()
  });
  page.open_file(
    "KON-12",
    "migrations/0042_delivery_attempts.sql",
    ChangeSource::Checkpoint,
  );
  showing(&mut page, "migrations/0042_delivery_attempts.sql");
  page.click("changes-open-file");
  let (file, at) = shown(&page);
  assert_eq!(file, "migrations/0042_delivery_attempts.sql");
  assert!(matches!(at, Version::Revision(_)), "the checkpoint: {at:?}");
  page.window.until("the file", |window| {
    window.tree.get_element_by_id("files-code").is_some()
  });
}
