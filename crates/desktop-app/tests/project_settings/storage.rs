//! Storage opens from Runs and the pinned item; its Keep free field shares the MCP invariant.
use orchester_core::{store::SettingsStore, workspaces::KeepFree};

use super::harness::Settings;

fn open(page: &mut Settings) {
  page.click("settings-storage-row");
  page.until("storage field", |p| p.exists("storage-keep-free"));
  page.until("disk loaded", |p| !p.value("storage-keep-free").is_empty());
  page.saved();
}
fn leave(page: &mut Settings) {
  page.window.press("storage-head");
}

#[test]
fn storage_keep_free_validates_and_saves_the_same_rule_as_mcp() {
  let mut page = Settings::new(false);
  open(&mut page);
  assert_eq!(page.value("storage-keep-free"), "50");
  for value in ["", "0"] {
    page.set("storage-keep-free", value);
    leave(&mut page);
    assert_eq!(page.db().keep_free().expect("read"), KeepFree::DEFAULT);
    assert!(page.text("storage-keep-row").contains("1 to 1048576"));
  }
  for value in ["1", "1048576"] {
    page.set("storage-keep-free", value);
    leave(&mut page);
    page.until("saved keep free", |p| {
      p.db().keep_free().expect("read").gigabytes().to_string() == value
    });
    page.saved();
    assert_eq!(page.value("storage-keep-free"), value);
  }
  page.set("storage-keep-free", "1048577");
  leave(&mut page);
  assert_eq!(page.db().keep_free().expect("read").gigabytes(), 1048576);
  assert!(page.text("storage-keep-row").contains("1 to 1048576"));
}

#[test]
fn storage_low_disk_item_opens_the_storage_subpage() {
  let mut page = Settings::new(false);
  page
    .db()
    .save_keep_free(KeepFree::from_gigabytes(KeepFree::MAX_GB).expect("valid"))
    .expect("save");
  page.until("low disk status item", |p| p.exists("status-low-disk"));
  assert!(page.text("status-low-disk").contains("new steps wait"));
  page.click("status-low-disk");
  page.until("storage", |p| p.exists("storage-crumb"));
  assert!(page.text("storage-crumb").contains("Storage"));
  assert!(page.text("storage-build-after-row").contains("not implemented"));
}
