//! With window control on, an MCP client may show Settings but cannot flip *Let automation change agent
//! permissions*: while its row is mounted, window input is refused as `person_only`, by element, by
//! coordinates and by keys, and the setting stays off. Without the row, input works again.

use orchester_core::time::Timestamp;
use serde_json::{Value, json};

use super::{
  Served,
  kontur::{self, SeedRequest, scenarios},
};

const SWITCH: &str = "settings-change-permissions";

/// Kontur seeded empty, served with window control on.
fn controlled() -> Served {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let request = SeedRequest {
    data_dir: folder.path().join("bin"),
    repo: folder.path().join("repo-empty"),
    scenario: scenarios::named("empty").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = kontur::seed(&request).expect("the scenario seeds");
  let runner = folder.path().join("no-runner").join("orchester-runner");
  let database = seeded.app_database;
  Served::start_with(folder, &database, runner, true)
}

impl Served {
  /// The middle of element `id`, in the window's pixels.
  fn middle(&mut self, id: &str) -> (f32, f32) {
    let bounds = self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0)
  }

  fn changes_permissions(&self) -> bool {
    self
      .services()
      .workspace
      .automation_changes_permissions()
      .expect("the setting reads")
  }
}

fn assert_person_only(error: &Value) {
  assert_eq!(error["code"], "person_only", "{error}");
}

#[test]
fn domain_window_input_cannot_flip_the_person_only_switch() {
  let mut served = controlled();
  served.ok("navigate", &json!({ "route": "settings" }));
  served.until("the switch to show", |served| {
    served.tree.get_element_by_id(SWITCH).is_some()
  });
  let found = served.ok("lurq_find_by_id", &json!({ "id": SWITCH }));
  let reference = found
    .as_str()
    .and_then(|line| line.split_whitespace().next())
    .unwrap_or_else(|| panic!("a ref: {found}"))
    .to_owned();
  assert_person_only(&served.refused("lurq_interact", &json!({ "action": "click", "ref": reference })));
  assert_person_only(&served.refused("lurq_act", &json!({ "ref": reference, "action": "invoke" })));
  assert_person_only(&served.refused("lurq_set_value", &json!({ "ref": reference, "value": true })));
  let (x, y) = served.middle(SWITCH);
  assert_person_only(&served.refused("lurq_interact", &json!({ "action": "click", "x": x, "y": y })));
  assert_person_only(&served.refused("lurq_interact", &json!({ "action": "key", "key": "Tab" })));
  assert_person_only(&served.refused("lurq_interact", &json!({ "action": "key", "key": " " })));
  assert_person_only(&served.refused("lurq_resize", &json!({ "width": 1440, "height": 900 })));
  served.ok("lurq_read_tree", &json!({}));
  served.ok("app_state", &json!({}));
  for _ in 0..5 {
    served.pass();
  }
  assert!(!served.changes_permissions(), "the setting stays off");
  served.ok("navigate", &json!({ "route": "all_projects" }));
  served.until("the switch to go", |served| {
    served.tree.get_element_by_id(SWITCH).is_none()
  });
  served.ok("lurq_interact", &json!({ "action": "move", "x": x, "y": y }));
}
