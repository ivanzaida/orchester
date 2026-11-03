//! A save reads the settings back right after writing them, on the window's thread, and the tab shows that reading at
//! once; the change it records reads the settings' query again off the window's thread (`queries`), so a change made
//! meanwhile is what the tab shows in the end. Switching tabs starts no read on the window's thread.

use std::{
  thread,
  time::{Duration, Instant},
};

use orchester_app::routes::{ProjectPage, Route, TaskList};
use orchester_core::{agent_files::RepositoryAgentFiles, store::SettingsStore};
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
fn project_settings_save_reads_the_settings_back_once_and_switching_tabs_reads_nothing() {
  let mut page = Settings::new(false);
  page.tab("context", "settings-agent-files");
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
  // Back to a tab shown before: its settings show from the session's cache.
  page.tab("general", "settings-project-name");
  page.tab("context", "settings-agent-files");
  settle(&mut page);
  let switched = reads.take();
  assert_eq!(switched.get("project settings"), None, "{switched:?}");
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
  page.tab("context", "settings-agent-files");
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
  page.tab("context", "settings-agent-files");
  settle(&mut page);
  let before = page.db().repository_agent_files().expect("setting").is_on();
  page.click("settings-agent-files");
  recorded_off_the_window(&page);
  // Automation puts the setting back after the save recorded its change, before the tab took the save.
  let back = if before {
    RepositoryAgentFiles::On
  } else {
    RepositoryAgentFiles::Off
  };
  page.db().save_repository_agent_files(back).expect("automation's write");
  page.outside(Outside::ProjectSettings);
  page.saved();
  page.until("the later change shown", |p| {
    p.switch_on("settings-agent-files") == before
  });
  settle(&mut page);
  assert_eq!(
    page.switch_on("settings-agent-files"),
    before,
    "the save's read did not stay"
  );
}
