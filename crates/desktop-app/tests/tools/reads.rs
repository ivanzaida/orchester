//! A saved tool reads the tools again once, though the save is recorded for every other screen too, and the
//! editor shows what was saved.

use std::{
  thread,
  time::{Duration, Instant},
};

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
  t.set("tool-timeout", "60");
  t.ready();
  settle(&mut t);
  let feed = t.window.projects().feed(project_id());
  let before = feed.outside.get_untracked().tools;
  let reads = Reads::count();
  t.click("tool-save");
  t.wait_state("Draft");
  settle(&mut t);
  let saved = reads.take();
  assert_eq!(saved.get("tools"), Some(&1), "{saved:?}");
  assert_eq!(feed.outside.get_untracked().tools, before + 1, "the save is recorded");
  assert_eq!(command_text(&t.entry(id).tool).timeout, "60");
}
