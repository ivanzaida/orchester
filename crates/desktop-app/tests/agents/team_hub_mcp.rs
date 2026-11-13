//! The panel of a team MCP server (`Agents · MCP servers` `yIsDi` A3, `C4` `SpRvd` … `C8` `lSA4x`, ORC-833): what the
//! team holds of it, the program path, the secret values and *On here* of this computer, how it stands and the pinned
//! fix of the first missing part. `cargo test -p orchester-app --test agents team_hub_mcp`.

use std::{path::Path, thread, time::Duration};

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{HubFilter, HubView};
use orchester_core::{
  id::{ResourceId, ServerId},
  resource::TeamMcpServer,
  secret::{SecretName, SecretValue},
};
use orchester_store::{AppDb, ProjectDb};
use serde_json::{Value, json};

use super::{
  harness::Agents,
  team_hub::{cached, open, team},
  team_hub_other::{as_member, bounds},
  team_hub_profile_fixes::Client,
};

const VALUE: &str = "pcl_not-a-real-token-4c1e";

/// How a server was switched here before the panel opens.
#[derive(Clone, Copy)]
pub(super) enum Switch {
  On,
  Undecided,
}

pub(super) fn program_body() -> Value {
  json!({"name": "pencil-design", "launch": {
    "kind": "program", "program": "pencil-mcp", "args": ["--stdio", "--app", "desktop"],
    "variables": ["PENCIL_TOKEN"]
  }, "exempt_tools": ["get_screenshot", "batch_get"]})
}

pub(super) fn remote_body() -> Value {
  json!({"name": "copilot", "launch": {
    "kind": "remote", "url": "https://api.githubcopilot.com/mcp/",
    "secret_headers": [{"header": "Authorization", "secret": "GITHUB_TOKEN", "prefix": "Bearer "}]
  }})
}

pub(super) fn server(agents: &Agents, (slug, body): (&str, Value), switch: Switch) -> ResourceId {
  let parsed: TeamMcpServer = serde_json::from_value(body.clone()).expect("a team MCP server");
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let id = cached(&mut db, (slug, slug), body, &parsed);
  match switch {
    Switch::On => db.set_team_mcp_on_here(id, true).expect("on"),
    Switch::Undecided => {}
  }
  id
}

fn name(text: &str) -> SecretName {
  SecretName::new(text).expect("secret name")
}

/// Gives the secret `text` a value in the window's keychain stand-in.
pub(super) fn give_value(agents: &Agents, text: &str) {
  let services = agents.window.launch.services().expect("services");
  ProjectDb::open(&agents.root)
    .expect("database")
    .set_team_secret_value(
      services.vault.get(),
      &name(text),
      &SecretValue::new(VALUE.to_owned()).expect("value"),
    )
    .expect("kept");
}

pub(super) fn team_server(agents: &Agents) -> ServerId {
  ProjectDb::open(&agents.root)
    .expect("database")
    .sharing()
    .expect("sharing")
    .expect("linked")
    .server
}

fn set_path(agents: &Agents, server: ResourceId, path: &Path) {
  AppDb::open(&agents.window.data.database())
    .expect("application database")
    .set_mcp_server_program(
      team_server(agents),
      server,
      path,
      orchester_core::time::Timestamp::from_unix_millis(5),
    )
    .expect("path");
}

pub(super) fn program_file(agents: &Agents) -> std::path::PathBuf {
  let path = agents.root.join("pencil-mcp");
  std::fs::write(&path, "#!/bin/sh\n").expect("a program");
  path
}

pub(super) fn show(agents: &mut Agents, server: ResourceId) {
  open(agents, HubView::new(HubFilter::McpServers, Some(server)));
  agents.until("the server panel", |s| {
    s.exists("team-hub-from-team") && s.exists("team-hub-here")
  });
}

pub(super) fn switch_of(agents: &Agents, server: ResourceId) -> Option<bool> {
  ProjectDb::open(&agents.root)
    .expect("database")
    .team_mcp_switch(server)
    .expect("read")
}

fn settle(agents: &mut Agents) {
  for _ in 0..20 {
    agents.window.tree.tick_futures();
    agents.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
}

#[test]
fn team_hub_mcp_c4_a_program_without_a_path_asks_for_it_and_says_what_the_team_holds() {
  let mut agents = Agents::new(true);
  team(&agents);
  let pencil = server(&agents, ("pencil-design", program_body()), Switch::On);
  give_value(&agents, "PENCIL_TOKEN");
  // What the team holds, as values: a member's panel (an admin's is the form).
  as_member(&agents);
  show(&mut agents, pencil);
  // From the team: how it runs, by names and never a path or a value.
  assert_eq!(agents.text("team-hub-panel-title"), "pencil-design");
  let version = agents.text("team-hub-panel-version");
  assert!(version.starts_with("v1 · published by Olena · "), "{version}");
  assert_eq!(
    agents.text("team-hub-from-team-runs-value"),
    "A program on each computer"
  );
  assert_eq!(agents.text("team-hub-from-team-program-value"), "pencil-mcp");
  assert_eq!(agents.text("team-hub-from-team-args-value"), "--stdio --app desktop");
  assert_eq!(agents.text("team-hub-from-team-folder-value"), "The run's worktree");
  assert_eq!(
    agents.text("team-hub-from-team-exempt-value"),
    "get_screenshot, batch_get"
  );
  assert_eq!(
    agents.text("team-hub-from-team-variables-value-0"),
    "PENCIL_TOKEN ← secret PENCIL_TOKEN"
  );
  // On this computer.
  assert_eq!(agents.text("team-hub-chip-here-label"), "Needs the program path here");
  assert_eq!(agents.text("team-hub-path-sub"), "pencil-mcp on this computer");
  assert_eq!(
    agents.text("team-hub-path-value-text"),
    "Not set · nothing is guessed or found by name"
  );
  assert_eq!(agents.text("team-hub-value-PENCIL_TOKEN-words"), "Value set here");
  assert_eq!(agents.text("team-hub-here-switch"), "On: runs here may use it");
  assert_eq!(
    agents.text("team-hub-mcp-note-text"),
    "Not ready here: set the program path. Runs whose pipeline uses pencil-design don't start on this computer until then."
  );
  assert_eq!(agents.text("team-hub-fix-set-path"), "Set program path");
  assert_eq!(
    agents.text("team-hub-actions-hint"),
    "Kept on this computer · asked once"
  );
  assert!(!agents.exists("team-hub-fix-check-again"));
}

#[test]
fn team_hub_mcp_c5_a_path_whose_file_is_gone_stays_marked_until_another_is_chosen() {
  let mut agents = Agents::new(true);
  team(&agents);
  let pencil = server(&agents, ("pencil-design", program_body()), Switch::On);
  give_value(&agents, "PENCIL_TOKEN");
  let gone = agents.root.join("tools").join("pencil-mcp");
  set_path(&agents, pencil, &gone);
  show(&mut agents, pencil);
  assert_eq!(agents.text("team-hub-chip-here-label"), "Program missing here");
  // The list's line says what the panel's chip says (`h2md5`).
  assert_eq!(
    agents.text(&format!("team-hub-row-{pencil}-line")),
    "Program missing here"
  );
  assert_eq!(agents.text("team-hub-path-sub"), "No file there");
  assert_eq!(agents.text("team-hub-path-value-text"), gone.display().to_string());
  let note = agents.text("team-hub-mcp-note-text");
  assert!(note.starts_with("Not ready here: no file at "), "{note}");
  assert!(note.ends_with("Runs that use pencil-design don't start until it is there again."));
  assert_eq!(agents.text("team-hub-fix-set-path"), "Choose another file");
  assert_eq!(agents.text("team-hub-fix-check-again"), "Check again");
  // The file comes back: Check again reads it.
  std::fs::create_dir_all(gone.parent().expect("folder")).expect("folder");
  std::fs::write(&gone, "#!/bin/sh\n").expect("the file is back");
  agents.click("team-hub-fix-check-again");
  agents.until("ready again", |s| s.text("team-hub-chip-here-label") == "Ready here");
  assert_eq!(agents.text("team-hub-fix-set-path"), "Change path");
}

#[test]
fn team_hub_mcp_c6_a_secret_without_a_value_is_added_in_a_dialog_by_a_person() {
  let mut agents = Agents::new(true);
  team(&agents);
  let pencil = server(&agents, ("pencil-design", program_body()), Switch::On);
  set_path(&agents, pencil, &program_file(&agents));
  show(&mut agents, pencil);
  assert_eq!(
    agents.text("team-hub-chip-here-label"),
    "Secret PENCIL_TOKEN has no value here"
  );
  assert!(agents.text("team-hub-path-sub").starts_with("found · "));
  assert_eq!(agents.text("team-hub-value-PENCIL_TOKEN-words"), "No value here");
  assert_eq!(
    agents.text("team-hub-mcp-note-text"),
    "Not ready here: PENCIL_TOKEN needs a value."
  );
  assert_eq!(agents.text("team-hub-fix-add-value"), "Add value");
  assert_eq!(
    agents.text("team-hub-actions-hint"),
    "Stored in this computer's secret store"
  );
  let client = Client::new(&mut agents);
  agents.press_until("team-hub-fix-add-value", "the dialog", |s| {
    s.exists("team-hub-secret-dialog")
  });
  agents.set("team-hub-secret-value", VALUE);
  assert!(
    !client.tree(&mut agents).contains(VALUE),
    "the typed value never reaches the tree"
  );
  agents.click("team-hub-secret-save");
  agents.until("kept", |s| {
    ProjectDb::open(&s.root)
      .expect("database")
      .has_team_secret_value(&name("PENCIL_TOKEN"))
      .expect("read")
  });
  agents.until("ready", |s| s.text("team-hub-chip-here-label") == "Ready here");
  assert!(!client.tree(&mut agents).contains(VALUE), "nor once it is kept");
}

#[test]
fn team_hub_mcp_c7_a_remote_server_has_no_path_and_shows_its_header() {
  let mut agents = Agents::new(true);
  team(&agents);
  let github = server(&agents, ("copilot", remote_body()), Switch::On);
  as_member(&agents);
  show(&mut agents, github);
  assert_eq!(
    agents.text("team-hub-from-team-url-value"),
    "https://api.githubcopilot.com/mcp/"
  );
  assert_eq!(
    agents.text("team-hub-from-team-header-0-value"),
    "Authorization: Bearer ← secret GITHUB_TOKEN"
  );
  assert!(!agents.exists("team-hub-path"), "a remote server has no program path");
  assert!(!agents.exists("team-hub-from-team-runs"));
  assert_eq!(
    agents.text("team-hub-chip-here-label"),
    "Secret GITHUB_TOKEN has no value here"
  );
  assert_eq!(agents.text("team-hub-fix-add-value"), "Add value");
}

#[test]
fn team_hub_mcp_c8_a_ready_server_offers_to_change_its_path() {
  let mut agents = Agents::new(true);
  team(&agents);
  let pencil = server(&agents, ("pencil-design", program_body()), Switch::On);
  let figma =
    json!({"name": "figma", "launch": {"kind": "remote", "url": "https://mcp.figma.test/", "secret_headers": []}});
  let ready = server(&agents, ("figma", figma), Switch::On);
  give_value(&agents, "PENCIL_TOKEN");
  set_path(&agents, pencil, &program_file(&agents));
  show(&mut agents, pencil);
  assert_eq!(agents.text("team-hub-chip-here-label"), "Ready here");
  assert_eq!(
    agents.text("team-hub-mcp-note-text"),
    "Ready here: runs on this computer may use pencil-design."
  );
  assert_eq!(agents.text("team-hub-fix-set-path"), "Change path");
  assert_eq!(agents.text("team-hub-actions-hint"), "Nothing missing on this computer");
  // A ready remote server has nothing to fix: an admin's bar is Save and Cancel, with nothing to save yet.
  open(&mut agents, HubView::new(HubFilter::McpServers, Some(ready)));
  agents.until("the remote panel", |s| s.exists("team-hub-edit-save"));
  assert!(!agents.exists("team-hub-fix-set-path"));
}

#[test]
fn team_hub_mcp_set_path_asks_the_picker_and_keeps_the_path_for_the_computer() {
  let mut agents = Agents::new(true);
  team(&agents);
  let pencil = server(&agents, ("pencil-design", program_body()), Switch::On);
  give_value(&agents, "PENCIL_TOKEN");
  show(&mut agents, pencil);
  let kept = |agents: &Agents| {
    AppDb::open(&agents.window.data.database())
      .expect("application database")
      .mcp_server_program(team_server(agents), pencil)
      .expect("read")
      .map(|set| set.program)
  };
  // Closed without a choice: nothing changes.
  agents.stand_ins.choose(None);
  agents.click("team-hub-choose-path");
  settle(&mut agents);
  assert_eq!(kept(&agents), None);
  let program = program_file(&agents);
  agents.stand_ins.choose(Some(program.clone()));
  agents.click("team-hub-choose-path");
  agents.until("kept", |s| kept(s) == Some(program.clone()));
  agents.until("ready", |s| s.text("team-hub-chip-here-label") == "Ready here");
  assert_eq!(agents.text("team-hub-path-value-text"), program.display().to_string());
}

#[test]
fn team_hub_mcp_automation_cannot_set_a_path_switch_on_or_add_a_value() {
  let mut agents = Agents::new(true);
  team(&agents);
  let pencil = server(&agents, ("pencil-design", program_body()), Switch::Undecided);
  let github = server(&agents, ("copilot", remote_body()), Switch::On);
  show(&mut agents, pencil);
  let program = program_file(&agents);
  agents.stand_ins.choose(Some(program));
  with_synthetic_input(|| agents.click("team-hub-choose-path"));
  assert!(agents.refused().is_some(), "refused to the client that pressed it");
  settle(&mut agents);
  assert!(
    AppDb::open(&agents.window.data.database())
      .expect("application database")
      .mcp_server_program(team_server(&agents), pencil)
      .expect("read")
      .is_none(),
    "automation set no path"
  );
  // The pinned fix of the first missing part is switching it on.
  assert_eq!(agents.text("team-hub-fix-switch-on"), "Turn on here");
  with_synthetic_input(|| agents.click("team-hub-fix-switch-on"));
  assert!(agents.refused().is_some(), "refused to the client that pressed it");
  settle(&mut agents);
  assert_eq!(switch_of(&agents, pencil), None, "automation switched nothing on");
  with_synthetic_input(|| agents.click("team-hub-here-switch"));
  assert!(agents.refused().is_some(), "refused to the client that pressed it");
  settle(&mut agents);
  assert_eq!(switch_of(&agents, pencil), None, "nor by the switch");
  // The person's own flip does.
  agents.click("team-hub-here-switch");
  agents.until("on", |s| {
    ProjectDb::open(&s.root)
      .expect("database")
      .team_mcp_switch(pencil)
      .expect("read")
      == Some(true)
  });
  agents.until("said on", |s| {
    s.text("team-hub-here-switch") == "On: runs here may use it"
  });
  // A secret without a value is the same: pressing Add value under automation opens no dialog, and is refused to it.
  show(&mut agents, github);
  agents.until("its panel", |s| s.text("team-hub-panel-title") == "copilot");
  assert_eq!(agents.text("team-hub-fix-add-value"), "Add value");
  with_synthetic_input(|| agents.click("team-hub-fix-add-value"));
  assert!(agents.refused().is_some(), "refused to the client that pressed it");
  settle(&mut agents);
  assert!(!agents.exists("team-hub-secret-dialog"), "automation opened no dialog");
  assert!(
    !agents.exists("team-hub-fix-person-only"),
    "and the panel says nothing of it"
  );
  // The person's own press does.
  agents.click("team-hub-fix-add-value");
  agents.until("the dialog", |s| s.exists("team-hub-secret-dialog"));
}

#[test]
fn team_hub_mcp_keys_are_170_wide_from_the_team_and_on_this_computer() {
  let mut agents = Agents::new(true);
  team(&agents);
  let pencil = server(&agents, ("pencil-design", program_body()), Switch::On);
  as_member(&agents);
  show(&mut agents, pencil);
  let mut key = |row: &str| {
    let (value, ..) = bounds(&mut agents, &format!("{row}-value"));
    let (label, ..) = bounds(&mut agents, &format!("{row}-label"));
    value - label
  };
  let (from_team, on_computer) = (key("team-hub-from-team-program"), key("team-hub-path"));
  assert!(
    (from_team - on_computer).abs() < 0.5,
    "from the team {from_team}, on this computer {on_computer}: the same key column (A3 `yIsDi`, M2 `dijUt`)"
  );
}

#[test]
fn team_hub_mcp_a_long_path_ends_in_an_ellipsis_inside_its_box() {
  let mut agents = Agents::new(true);
  team(&agents);
  let pencil = server(&agents, ("pencil-design", program_body()), Switch::On);
  let gone = agents
    .root
    .join("a".repeat(60))
    .join("b".repeat(60))
    .join("c".repeat(60))
    .join("pencil-mcp");
  set_path(&agents, pencil, &gone);
  show(&mut agents, pencil);
  let (_, text, _) = bounds(&mut agents, "team-hub-path-value-text");
  let (_, boxed, _) = bounds(&mut agents, "team-hub-path-value");
  assert!(text < boxed, "the path keeps to its box: {text} in {boxed}");
}

#[test]
fn team_hub_mcp_a_long_found_path_ends_in_an_ellipsis_inside_its_row() {
  let mut agents = Agents::new(true);
  team(&agents);
  let pencil = server(&agents, ("pencil-design", program_body()), Switch::On);
  let folder = agents.root.join("a".repeat(60)).join("b".repeat(60));
  std::fs::create_dir_all(&folder).expect("a long folder");
  let program = folder.join("pencil-mcp");
  std::fs::write(&program, "#!/bin/sh\n").expect("a program");
  set_path(&agents, pencil, &program);
  show(&mut agents, pencil);
  assert!(agents.text("team-hub-path-sub").starts_with("found · "));
  let (x, width, _) = bounds(&mut agents, "team-hub-path-value-text");
  let (row_x, row_width, _) = bounds(&mut agents, "team-hub-path");
  assert!(
    x + width <= row_x + row_width,
    "the found path keeps to its row (C6 `LiS1F`, C8 `lSA4x`): ends at {}, the row at {}",
    x + width,
    row_x + row_width
  );
}

#[test]
fn team_hub_mcp_switching_off_is_allowed_to_automation_and_leaves_the_server_off_here() {
  let mut agents = Agents::new(true);
  team(&agents);
  let github = server(&agents, ("copilot", remote_body()), Switch::On);
  give_value(&agents, "GITHUB_TOKEN");
  show(&mut agents, github);
  assert_eq!(agents.text("team-hub-chip-here-label"), "Ready here");
  with_synthetic_input(|| agents.click("team-hub-here-switch"));
  agents.until("off", |s| {
    ProjectDb::open(&s.root)
      .expect("database")
      .team_mcp_switch(github)
      .expect("read")
      == Some(false)
  });
  agents.until("said off", |s| {
    s.text("team-hub-here-switch") == "Off: runs here don't use it"
  });
  assert_eq!(agents.refused(), None, "switching off is automation's too");
  assert_eq!(
    agents.text("team-hub-mcp-note-text"),
    "Off here: runs on this computer don't use copilot."
  );
  assert_eq!(agents.text("team-hub-chip-here-label"), "Off here");
}

#[test]
fn team_hub_mcp_a_server_nobody_decided_on_asks_the_person() {
  let mut agents = Agents::new(true);
  team(&agents);
  let github = server(&agents, ("copilot", remote_body()), Switch::Undecided);
  give_value(&agents, "GITHUB_TOKEN");
  show(&mut agents, github);
  assert_eq!(
    agents.text("team-hub-here-switch"),
    "Not decided yet: turn it on to let runs here use it"
  );
  assert_eq!(
    agents.text("team-hub-mcp-note-text"),
    "Not ready here: switch copilot on or off for this computer."
  );
  assert_eq!(agents.text("team-hub-fix-switch-on"), "Turn on here");
  agents.click("team-hub-fix-switch-on");
  agents.until("ready", |s| s.text("team-hub-chip-here-label") == "Ready here");
}
