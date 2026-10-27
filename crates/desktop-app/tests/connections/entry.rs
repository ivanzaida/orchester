//! The fields that add pills (`Models` `ZgOvN` in `10b`, `Run without asking` `hbex3` in `11`): Enter in the field adds
//! what it holds as *Add* does, each value is a 24-high pill, and a note under them says how; and *or New secret…*
//! under API key (`Z6Pq8I`) chooses a new secret, its name and key opening under it.

use orchester_app::routes::NewConnection;

use super::harness::Seeded;

/// A pill's height (`Pill · deepseek-v4-pro` `TnP0h`).
const PILL_HEIGHT: f32 = 24.0;

/// Presses Enter in the field `id`, as a person does after typing.
fn enter_in(seeded: &mut Seeded, id: &str) {
  let tree = &mut seeded.window.tree;
  tree
    .get_element_by_id_mut(id)
    .unwrap_or_else(|| panic!("{id} is shown"))
    .focus();
  seeded.window.pass();
  let tree = &mut seeded.window.tree;
  tree.key_down("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  tree.key_up("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  seeded.window.pass();
}

fn height(seeded: &mut Seeded, id: &str) -> f32 {
  seeded
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
    .height
}

#[test]
fn connections_entry_enter_adds_a_model_and_new_secret_opens_its_fields() {
  let mut seeded = Seeded::new("connections");
  seeded.add(NewConnection::AgentProvider);
  seeded.click("connection-kind-claude");
  seeded.click("connection-runs-provider");
  assert_eq!(
    seeded.text("connection-provider-models-note"),
    "Enter adds it; × removes one."
  );
  seeded.set("connection-provider-model", "deepseek-v4-pro");
  enter_in(&mut seeded, "connection-provider-model");
  assert_eq!(seeded.text("connection-provider-model-0-label"), "deepseek-v4-pro");
  assert!(
    (height(&mut seeded, "connection-provider-model-0") - PILL_HEIGHT).abs() < 0.5,
    "a model is a 24-high pill"
  );
  assert!(!seeded.exists("connection-provider-secret-name"));
  assert_eq!(seeded.text("connection-provider-new-secret"), "or New secret…");
  seeded.click("connection-provider-new-secret");
  assert!(
    seeded.exists("connection-provider-secret-name"),
    "the new secret's name and key open"
  );
  assert!(
    !seeded.exists("connection-provider-new-secret"),
    "a new secret is chosen already"
  );
}

#[test]
fn connections_entry_enter_adds_a_tool_that_runs_without_asking() {
  let mut seeded = Seeded::new("connections");
  seeded.add(NewConnection::McpServer);
  assert_eq!(
    seeded.text("connection-mcp-exempt-tools-note"),
    "Tool names as the server names them; every other call asks."
  );
  seeded.set("connection-mcp-exempt-tools", "get_figma_data");
  enter_in(&mut seeded, "connection-mcp-exempt-tools");
  assert_eq!(seeded.text("connection-mcp-tool-0-label"), "get_figma_data");
  assert!(
    (height(&mut seeded, "connection-mcp-tool-0") - PILL_HEIGHT).abs() < 0.5,
    "a tool is a 24-high pill"
  );
  assert!(
    !seeded.exists("connection-confirm-save-confirm"),
    "Enter adds the tool and asks nothing about saving"
  );
}
