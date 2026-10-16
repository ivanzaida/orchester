//! A save reads the settings once, right after writing them, and the tab shows that reading; a change made outside
//! the tab still reads them again.

use std::{
  thread,
  time::{Duration, Instant},
};

use orchester_app::routes::{ProjectPage, Route, TaskList};
use orchester_core::store::SettingsStore;
use orchester_workspace::tasks::outside::Outside;

use super::{
  harness::Settings,
  support::{project_id, reads::Reads},
};

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

/// How often the project's settings changed, as the session counts it.
fn project_settings_count(page: &Settings) -> u64 {
  let services = page.window.launch.services().expect("services");
  services.changes.counts(project_id()).project_settings
}

/// Waits, delivering nothing to the window, until the save running off the UI thread recorded its change.
fn recorded_off_the_window(page: &Settings) {
  let deadline = Instant::now() + Duration::from_secs(10);
  while project_settings_count(page) == 0 {
    assert!(Instant::now() < deadline, "the save recorded no change");
    thread::sleep(Duration::from_millis(5));
  }
}

#[test]
fn project_settings_save_is_recorded_when_the_tab_leaves_before_it_ends() {
  let mut page = Settings::new(false);
  settle(&mut page);
  let before = page.db().repository_agent_files().expect("setting").is_on();
  page.click("settings-agent-files");
  // The person leaves before the save ended: the tab that started it is gone when it does.
  page
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Tasks(TaskList::default())));
  page.window.pass();
  page.until("the write", |p| {
    p.db().repository_agent_files().expect("saved").is_on() != before
  });
  page.until("the change recorded", |p| project_settings_count(p) == 1);
  let feed = page.window.projects().feed(project_id());
  page.until("the feed told", |_| feed.outside.get_untracked().project_settings == 1);
}

#[test]
fn project_settings_save_does_not_take_a_change_made_while_it_ran() {
  let mut page = Settings::new(false);
  settle(&mut page);
  let reads = Reads::count();
  page.click("settings-agent-files");
  recorded_off_the_window(&page);
  // Automation changes the settings after the save recorded its change, before the tab took the save.
  page.outside(Outside::ProjectSettings);
  page.saved();
  page.until("the later change read", |_| reads.total() >= 2);
  settle(&mut page);
  let counted = reads.take();
  assert_eq!(
    counted.get("project settings"),
    Some(&2),
    "the save's own read, and one for the later change: {counted:?}"
  );
}
