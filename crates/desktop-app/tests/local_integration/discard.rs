//! *Discard selected* on the Local integration page: it asks in place, then discards to the window's trash (a folder of
//! the test's own), the list empties and delivery goes on; window automation can neither open nor confirm it.
use std::{fs, path::PathBuf, sync::Arc};

use lurq::app::events::with_synthetic_input;
use orchester_core::integration::local::{LocalIntegrationStore, LocalPhase};
use orchester_store::ProjectDb;
use orchester_workspace::accounts::trash::FolderTrash;

use super::{domain_support::Fixture, integration_support::Page, support::Window};

const JUNK: &str = "du.exe.stackdump";
const SELECT: &str = "local-select-local_integration.untracked-du.exe.stackdump";
const ROW: &str = "local-row-local_integration.untracked-du.exe.stackdump";

/// The page of a run whose main checkout holds an untracked crash dump, its window with a trash folder of its own.
fn junk_page() -> (Page, tempfile::TempDir) {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join(JUNK), b"crash dump").expect("junk");
  let mut page = Page::new(fixture);
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
  let bin = tempfile::Builder::new()
    .prefix("trash-")
    .tempdir_in(scratch)
    .expect("trash");
  page
    .window
    .launch
    .workspace()
    .expect("workspace")
    .use_trash(Arc::new(FolderTrash::new(bin.path())));
  page.ready("local-review-discard");
  (page, bin)
}

fn phase(page: &Page) -> Option<LocalPhase> {
  ProjectDb::open(page.root())
    .expect("db")
    .local_operation(page.run())
    .expect("journal")
    .map(|operation| operation.phase())
}

fn refused(window: &mut Window) -> bool {
  window
    .texts("local-work-error")
    .is_some_and(|texts| texts.join(" ").contains("Only a person can discard"))
}

#[test]
fn local_integration_discard_asks_then_discards_and_delivery_resumes() {
  let (mut page, bin) = junk_page();
  page.click(SELECT);
  page.click("local-review-discard");
  page.ready("local-discard-keep");
  page.click("local-discard-keep");
  page.ready("local-review-discard");
  assert!(page.root().join(JUNK).exists(), "keeping it changes nothing");
  page.click("local-review-discard");
  page.ready("local-discard-confirm");
  assert!(
    page
      .window
      .text("local-discard-files")
      .contains("du.exe.stackdump moves to the")
  );
  assert_eq!(page.window.text("local-discard-confirm"), "Discard 1 file");
  assert!(page.root().join(JUNK).exists(), "asking changes nothing");
  page.click("local-discard-confirm");
  page.ready("local-sync");
  assert!(!page.root().join(JUNK).exists());
  assert_eq!(fs::read(bin.path().join(JUNK)).expect("in the trash"), b"crash dump");
  assert!(!page.has(ROW), "the list no longer has it");
  assert_eq!(phase(&page), Some(LocalPhase::Prepared));
}

#[test]
fn local_integration_discard_refuses_automation_opening_it() {
  let (mut page, bin) = junk_page();
  page.window.link.input_gate().bind_window_control(true);
  page.click(SELECT);
  with_synthetic_input(|| page.click("local-review-discard"));
  page.until("the refusal", refused);
  assert!(!page.has("local-discard-confirm"));
  assert!(page.root().join(JUNK).exists());
  assert!(fs::read_dir(bin.path()).expect("trash").next().is_none());
}

#[test]
fn local_integration_discard_refuses_automation_confirming_it() {
  let (mut page, bin) = junk_page();
  page.window.link.input_gate().bind_window_control(true);
  page.click(SELECT);
  page.click("local-review-discard");
  page.ready("local-discard-confirm");
  assert!(page.window.link.input_gate().protecting());
  with_synthetic_input(|| page.click("local-discard-confirm"));
  page.until("the refusal", refused);
  assert!(page.has("local-discard-confirm"), "the question stays for the person");
  assert!(page.root().join(JUNK).exists());
  assert!(fs::read_dir(bin.path()).expect("trash").next().is_none());
  assert_ne!(phase(&page), Some(LocalPhase::Prepared), "nothing was discarded");
}

/// A file another program holds: the page says it stayed, in the system's words, and *Try again* discards it once the
/// program lets go.
#[cfg(windows)]
#[test]
fn local_integration_discard_says_which_file_stayed_and_tries_again() {
  use std::os::windows::fs::OpenOptionsExt;

  let (mut page, _bin) = junk_page();
  // Reading and writing are shared, moving is not.
  let held = fs::OpenOptions::new()
    .read(true)
    .share_mode(0x1 | 0x2)
    .open(page.root().join(JUNK))
    .expect("held");
  page.click(SELECT);
  page.click("local-review-discard");
  page.ready("local-discard-confirm");
  page.click("local-discard-confirm");
  page.ready("local-discard-again");
  let said = page.window.text("local-discarded-files");
  assert!(said.contains("du.exe.stackdump: not discarded."), "{said}");
  assert!(said.contains("os error 32"), "the system's reason: {said}");
  assert!(page.root().join(JUNK).exists());
  drop(held);
  page.click("local-discard-again");
  page.ready("local-sync");
  assert!(!page.root().join(JUNK).exists());
}
