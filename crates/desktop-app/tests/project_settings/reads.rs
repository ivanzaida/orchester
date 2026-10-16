//! A save reads the settings once, right after writing them, and the tab shows that reading; a change made outside
//! the tab still reads them again.

use std::{
  thread,
  time::{Duration, Instant},
};

use orchester_core::store::SettingsStore;
use orchester_workspace::tasks::outside::Outside;

use super::{harness::Settings, support::reads::Reads};

/// Delivers what the window's futures finish for a while, so every read a change starts is counted.
fn settle(page: &mut Settings) {
  let until = Instant::now() + Duration::from_millis(500);
  while Instant::now() < until {
    page.window.tree.tick_futures();
    page.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
}

#[test]
fn project_settings_save_reads_the_settings_once() {
  let mut page = Settings::new(false);
  settle(&mut page);
  let reads = Reads::count();
  let before = page.db().repository_agent_files().expect("setting").is_on();
  page.click("settings-agent-files");
  page.until("agent-file save", |p| {
    p.db().repository_agent_files().expect("saved").is_on() != before
  });
  page.saved();
  settle(&mut page);
  let saved = reads.take();
  assert_eq!(saved.get("project settings"), Some(&1), "{saved:?}");
  page.outside(Outside::ProjectSettings);
  page.until("the change read", |_| reads.total() > 0);
  settle(&mut page);
  let outside = reads.take();
  assert_eq!(outside.get("project settings"), Some(&1), "{outside:?}");
}
