//! A profile saved on the screen is recorded as automation's changes are, so every other screen that shows the
//! profiles reads them again, while this one reads them again once.

use std::{
  thread,
  time::{Duration, Instant},
};

use super::{
  harness::Agents,
  support::{project_id, reads::Reads},
};

#[test]
fn agents_save_is_recorded_and_read_again_once() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.set("agent-responsibility", "Checks the complete change");
  let feed = screen.window.projects().feed(project_id());
  let before = feed.outside.get_untracked().profiles;
  let reads = Reads::count();
  screen.click("agent-save");
  screen.until("saved", |s| s.text("screen-agents").contains("Draft saved"));
  let until = Instant::now() + Duration::from_millis(500);
  while Instant::now() < until {
    screen.window.tree.tick_futures();
    screen.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
  assert_eq!(
    feed.outside.get_untracked().profiles,
    before + 1,
    "the save is recorded"
  );
  let counted = reads.take();
  assert_eq!(counted.get("agents"), Some(&1), "{counted:?}");
}
