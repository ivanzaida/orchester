//! The list's layout by window width: compact rows without a header in a narrow window
//! (`Connections · All connections · 960` `Halhq`, `b3aoUZ`), a table with columns in a wide one (`z4QquH`), and
//! rows that scroll in a short window under the note that stays at the column's foot.

use lurq::{app::events::ScrollPhase, core::ElementRect};
use orchester_app::routes::ConnectionsTab;
use orchester_core::store::{ConnectionStore, McpServerStore};

use super::harness::Seeded;

/// The narrowest a row's name may be laid out: a compact row's name takes its text column (182 at 960),
/// a wide row's is as wide as its words (the seed's shortest, `sentry`, is about 39).
const COMPACT_NAME_MIN_WIDTH: f32 = 150.0;
const WIDE_NAME_MIN_WIDTH: f32 = 30.0;

/// The connections the seed made, by row id, with their names.
fn rows(seeded: &Seeded) -> Vec<(String, String)> {
  let db = seeded.db();
  let connections = db.connections().expect("connections read");
  let servers = db.mcp_servers().expect("servers read");
  connections
    .iter()
    .map(|connection| (Seeded::row(connection.id()), connection.name().as_str().to_owned()))
    .chain(
      servers
        .iter()
        .map(|server| (Seeded::row(server.id()), server.name().as_str().to_owned())),
    )
    .collect()
}

fn bounds(seeded: &mut Seeded, id: &str) -> ElementRect {
  seeded
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

fn at_size(scenario: &str, (width, height): (u32, u32)) -> Seeded {
  let mut seeded = Seeded::new(scenario);
  seeded.window.tree.resize(width, height);
  seeded.list(ConnectionsTab::All);
  seeded.window.pass();
  seeded
}

/// Every row's name is laid out inside the list, at least `min_width` wide, and says the name.
fn assert_names_shown(seeded: &mut Seeded, min_width: f32, at: &str) {
  let list = bounds(seeded, "connections-list");
  let rows = rows(seeded);
  assert_eq!(rows.len(), 6, "the scenario's connections");
  for (row, name) in rows {
    let id = format!("{row}-name");
    let cell = bounds(seeded, &id);
    assert!(cell.width >= min_width, "{name} at {at}: {cell:?}");
    assert!(
      cell.x >= list.x && cell.x + cell.width <= list.x + list.width + 0.5,
      "{name} inside the list at {at}: {cell:?} in {list:?}"
    );
    assert_eq!(seeded.text(&id), name, "at {at}");
  }
}

#[test]
fn connections_at_960_show_compact_rows_with_their_names() {
  let mut seeded = at_size("connections", (960, 900));
  assert_names_shown(&mut seeded, COMPACT_NAME_MIN_WIDTH, "960");
  assert!(!seeded.exists("connections-list-header"), "no header at 960");
  assert!(!seeded.exists("connections-notice"), "no notice at 960");
  let list = seeded.text("connections-list");
  for words in ["Anthropic · Claude Code CLI 2.1.89", "Sign-in required", "Turned off"] {
    assert!(list.contains(words), "{words} in {list}");
  }
  assert!(
    !list.contains("5 agents"),
    "what uses it is left to the inspector: {list}"
  );
  let (list, inspector) = (
    bounds(&mut seeded, "connections-list"),
    bounds(&mut seeded, "connection-inspector"),
  );
  // The inspector keeps its width; the page's sides and the gap beside it narrow (`b3aoUZ`).
  assert!((inspector.width - 360.0).abs() < 0.5, "{inspector:?}");
  assert!((list.width - 268.0).abs() < 0.5, "{list:?}");
}

#[test]
fn connections_at_1440_keep_the_table_with_columns() {
  let mut seeded = at_size("connections", (1440, 900));
  assert_names_shown(&mut seeded, WIDE_NAME_MIN_WIDTH, "1440");
  assert!(seeded.exists("connections-list-header"));
  assert!(seeded.exists("connections-notice"));
  let list = seeded.text("connections-list");
  for words in ["CONNECTION", "5 agents", "Provider"] {
    assert!(list.contains(words), "{words} in {list}");
  }
  let (row, _) = rows(&seeded).remove(0);
  let row = bounds(&mut seeded, &row);
  assert!((row.height - 56.0).abs() < 0.5, "{row:?}");
  let (list, inspector) = (
    bounds(&mut seeded, "connections-list"),
    bounds(&mut seeded, "connection-inspector"),
  );
  assert!((inspector.width - 360.0).abs() < 0.5, "{inspector:?}");
  assert!(
    (list.width - 728.0).abs() < 0.5,
    "the list as wide as z4QquH's: {list:?}"
  );
}

#[test]
fn connections_rows_scroll_in_a_short_window_and_the_note_stays_in_view() {
  const HEIGHT: f32 = 640.0;
  let mut seeded = at_size("api-providers", (960, 640));
  let last = Seeded::row(seeded.server("sentry").expect("sentry").id());
  let bottom = |seeded: &mut Seeded, id: &str| {
    let bounds = bounds(seeded, id);
    bounds.y + bounds.height
  };
  let note = bounds(&mut seeded, "connections-ownership");
  assert!(
    note.height > 0.0 && note.y + note.height <= HEIGHT,
    "the note in view: {note:?}"
  );
  let scroll = bounds(&mut seeded, "connections-list-scroll");
  assert!(
    bottom(&mut seeded, &last) > scroll.y + scroll.height,
    "the last row below the rows' room before scrolling"
  );
  let (x, y) = (scroll.x + scroll.width / 2.0, scroll.y + scroll.height / 2.0);
  seeded.window.tree.mouse_move(x, y);
  seeded.window.tree.scroll(x, y, 0.0, -4000.0, ScrollPhase::Scroll);
  seeded.window.pass();
  seeded.window.pass();
  assert!(
    bottom(&mut seeded, &last) <= scroll.y + scroll.height + 0.5,
    "scrolled into the rows' room"
  );
  let note = bounds(&mut seeded, "connections-ownership");
  assert!(note.y + note.height <= HEIGHT, "the note stays in view: {note:?}");
}
