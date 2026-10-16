//! Work on a connection records the change as automation's are: the list reads the connections again once, and
//! the status bar reads again the accounts the project uses.

use std::{
  thread,
  time::{Duration, Instant},
};

use orchester_app::routes::ConnectionsTab;

use super::{harness::Seeded, support::reads::Reads};

/// Delivers what the window's futures finish for a while, so every read a change starts is counted.
fn settle(seeded: &mut Seeded) {
  let until = Instant::now() + Duration::from_millis(500);
  while Instant::now() < until {
    seeded.window.tree.tick_futures();
    seeded.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
}

#[test]
fn connections_work_reads_the_list_once_and_the_status_bar_again() {
  let mut seeded = Seeded::new("connections");
  seeded.list(ConnectionsTab::McpServers);
  let docs = seeded.server("kontur-docs").expect("kontur-docs");
  seeded.click(&Seeded::row(docs.id()));
  settle(&mut seeded);
  let reads = Reads::count();
  seeded.click("connection-turn-off");
  seeded.click("connection-confirm-turn-off");
  seeded.until("the server turned off", |seeded| seeded.exists("connection-turn-on"));
  settle(&mut seeded);
  let counted = reads.take();
  assert_eq!(counted.get("connections"), Some(&1), "the list reads once: {counted:?}");
  assert_eq!(
    counted.get("used accounts"),
    Some(&1),
    "the status bar reads what the project uses: {counted:?}"
  );
}
