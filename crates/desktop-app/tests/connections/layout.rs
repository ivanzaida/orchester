//! Connections as a tab of Project settings (`05 Connections` `dk46o`): its groups in the page's 720 column at any
//! window width, every row's name inside it, and a connection's page with its head's actions on one line.

use lurq::core::ElementRect;
use orchester_core::store::{ConnectionStore, McpServerStore};

use super::harness::Seeded;

/// The page's column (`Body` `mBTu6`).
const COLUMN: f32 = 720.0;

/// The connections the seed made, by row id, with their names.
fn rows(seeded: &Seeded) -> Vec<(String, String)> {
  let db = seeded.db();
  let connections = db.connections().expect("connections read");
  let servers = db.mcp_servers().expect("servers read");
  connections
    .iter()
    .map(|connection| (connection.id(), connection.name().as_str().to_owned()))
    .chain(
      servers
        .iter()
        .map(|server| (server.id(), server.name().as_str().to_owned())),
    )
    .map(|(id, name)| (format!("connection-row-{id}"), name))
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
  seeded.list();
  seeded.window.pass();
  seeded
}

#[test]
fn connections_groups_keep_the_column_and_their_names_at_960_and_1440() {
  for width in [960, 1440] {
    let mut seeded = at_size("connections", (width, 1300));
    let list = bounds(&mut seeded, "connections-list");
    assert!(list.width <= COLUMN + 0.5, "at {width}: {list:?}");
    let rows = rows(&seeded);
    assert_eq!(rows.len(), 6, "the scenario's connections");
    for (row, name) in rows {
      let label = bounds(&mut seeded, &format!("{row}-label"));
      assert!(
        label.x >= list.x && label.x + label.width <= list.x + list.width + 0.5,
        "{name} inside the column at {width}: {label:?} in {list:?}"
      );
      assert_eq!(seeded.text(&format!("{row}-label")), name, "at {width}");
    }
  }
}

#[test]
fn connections_page_head_actions_stay_on_one_line_at_960_and_1440() {
  for width in [960, 1440] {
    let mut seeded = at_size("connections", (width, 1180));
    let codex = seeded
      .db()
      .connections()
      .expect("connections read")
      .into_iter()
      .find(|connection| connection.name().as_str() == "Codex · Kontur")
      .expect("the seed's Codex connection");
    seeded.open(codex.id());
    let page = bounds(&mut seeded, "connection-inspector");
    let (test, more) = (
      bounds(&mut seeded, "connection-test"),
      bounds(&mut seeded, "connection-more"),
    );
    assert!(
      (test.y + test.height / 2.0 - more.y - more.height / 2.0).abs() < 0.5,
      "one line at {width}"
    );
    assert!(
      more.x >= test.x + test.width && more.x + more.width <= page.x + page.width + 0.5,
      "side by side inside the page at {width}: {test:?}, {more:?} in {page:?}"
    );
  }
}
