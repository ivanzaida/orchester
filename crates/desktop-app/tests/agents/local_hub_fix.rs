//! A local MCP server's panel pins the fix of what it lacks while nothing is changed (L2 `IpzZ7`), as a profile's and
//! a team server's panels do: *Set program path* asks for the program's file, and what it takes is a change the panel
//! holds under *Save* and *Cancel*. `cargo test -p orchester-app --test agents local_hub_fix`.

use super::{
  harness::Agents,
  local_hub::{SAVE, figma, server, show},
};

const SET_PATH: &str = "local-hub-mcp-fix-set-path";

#[test]
fn local_hub_fix_a_server_without_its_program_pins_set_program_path() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  show(&mut agents, docs);
  agents.until("the fix", |s| s.exists(SET_PATH));
  assert_eq!(agents.text(SET_PATH), "Set program path");
  assert!(!agents.exists(SAVE), "nothing is changed: the bar is the fix's");

  let program = std::env::current_exe().expect("the test program");
  agents.stand_ins.choose(Some(program.clone()));
  agents.press_until(SET_PATH, "the file taken", |s| s.exists(SAVE));
  assert!(!agents.exists(SET_PATH), "a change makes the bar Save's and Cancel's");
  assert_eq!(agents.value("connection-mcp-program"), program.to_string_lossy());
}

#[test]
fn local_hub_fix_a_secret_without_a_value_pins_add_a_value() {
  let mut agents = Agents::new(true);
  let figma = figma(&agents);
  show(&mut agents, figma);
  let add = "local-hub-mcp-fix-add-value";
  agents.until("the fix", |s| s.exists(add));
  assert_eq!(agents.text(add), "Add a value for FIGMA_TOKEN");
  agents.press_until(add, "its dialog", |s| s.exists("local-hub-secret-value"));
}
