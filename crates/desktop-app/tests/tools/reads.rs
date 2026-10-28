//! A saved tool reads the tools again once, though the save is recorded for every other screen too, and the
//! editor shows what was saved.

use std::{
  thread,
  time::{Duration, Instant},
};

use orchester_app::routes::{ProjectPage, Route, TaskList};

use super::{
  harness::{Tools, command_text},
  support::{project_id, reads::Reads},
};

/// Delivers what the window's futures finish for a while, so every read a change starts is counted.
fn settle(t: &mut Tools) {
  let until = Instant::now() + Duration::from_millis(500);
  while Instant::now() < until {
    t.window.tree.tick_futures();
    t.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
}

#[test]
fn tools_save_reads_the_tools_once() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  t.edit(id);
  settle(&mut t);
  let feed = t.window.projects().feed(project_id());
  let before = feed.outside.get_untracked().tools;
  let reads = Reads::count();
  t.set("tool-timeout", "60");
  t.wait_state("Draft");
  settle(&mut t);
  let saved = reads.take();
  assert_eq!(saved.get("tools"), Some(&1), "{saved:?}");
  assert_eq!(feed.outside.get_untracked().tools, before + 1, "the save is recorded");
  assert_eq!(command_text(&t.entry(id).tool).timeout, "60");
}

#[test]
fn tools_save_is_recorded_when_the_editor_leaves_before_it_ends() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  t.edit(id);
  t.set("tool-timeout", "60");
  let services = t.window.launch.services().expect("the session started").clone();
  // The draft saves on its own once the program checks out.
  t.window.until("the save started", |w| {
    w.tree.tick_futures();
    w.pass();
    w.texts("tool-draft-bar-status")
      .is_some_and(|s| s.join(" ").contains("Saving"))
      || services.changes.counts(project_id()).tools == 1
  });
  // The person leaves before the save ended: the editor that started it is gone when it does.
  t.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Tasks(TaskList::default())));
  t.window.pass();
  t.window.until("the change recorded", |_| {
    services.changes.counts(project_id()).tools == 1
  });
  assert_eq!(command_text(&t.entry(id).tool).timeout, "60");
}
