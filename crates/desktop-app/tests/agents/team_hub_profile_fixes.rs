//! The fixes of the profile panel (ORC-832 review): the ones that widen what agents may do are a person's alone, a
//! secret's value is held safely and kept by its name, a pick in the select is kept, and the panel is one per
//! profile. `cargo test -p orchester-app --test agents team_hub_profile_fix`.

use std::{sync::Arc, thread, time::Duration};

use lurq::{
  app::events::with_synthetic_input,
  mcp::rmcp::{RoleClient, ServiceExt, model::CallToolRequestParams, service::RunningService},
};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  mcp::{self, McpSettings},
  routes::{HubFilter, HubView},
};
use orchester_core::{id::ResourceId, secret::SecretName};
use orchester_store::{ConnectionPick, ProjectDb};
use serde_json::{Value, json};

use super::{
  harness::Agents,
  team_hub::{open, team},
  team_hub_profile::{builder_using, claude, show},
};

const VALUE: &str = "ghp_not-a-real-token-9f2c";

/// The profile with a program server `lint` (no path yet) and the secret `GITHUB_TOKEN` (no value yet), on a computer
/// with one Claude Code connection. The profile's id and `lint`'s.
fn needing_path_and_value() -> (Agents, ResourceId, ResourceId) {
  let mut agents = Agents::new(true);
  team(&agents);
  claude(&agents.root, "Claude 2", "2.1.173");
  let ((profile, _), lint) = builder_using(&agents, true);
  show(&mut agents, profile);
  (agents, profile, lint.expect("lint"))
}

fn token() -> SecretName {
  SecretName::new("GITHUB_TOKEN").expect("name")
}

fn kept(agents: &Agents) -> bool {
  ProjectDb::open(&agents.root)
    .expect("database")
    .has_team_secret_value(&token())
    .expect("read")
}

/// What an MCP client reads of the window.
pub(super) struct Client {
  tools: RunningService<RoleClient, ()>,
  server: tokio::task::JoinHandle<()>,
}

impl Drop for Client {
  fn drop(&mut self) {
    self.server.abort();
  }
}

impl Client {
  pub(super) fn new(agents: &mut Agents) -> Self {
    let window = &mut agents.window;
    let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
    let config = mcp::config(
      McpSettings::with_window_control(true),
      &window.link,
      &appearance,
      window.launch.services(),
      window.app.i18n(),
    );
    let tools = window.tree.enable_mcp(config.hosted());
    let gate = window.link.input_gate();
    gate.attach(tools.clone());
    let (client, server) = tokio::io::duplex(64 * 1024);
    let server = window.data.runtime.spawn(async move {
      gate
        .guard(tools.service())
        .serve(server)
        .await
        .expect("server initializes")
        .waiting()
        .await
        .expect("server ends");
    });
    let tools = window
      .data
      .runtime
      .block_on(().serve(client))
      .expect("client initializes");
    Self { tools, server }
  }

  /// The whole tree, as the window's MCP server gives it.
  pub(super) fn tree(&self, agents: &mut Agents) -> String {
    let peer = self.tools.peer().clone();
    let params = CallToolRequestParams::new("lurq_read_tree".to_owned())
      .with_arguments(json!({}).as_object().expect("arguments").clone());
    let window = &mut agents.window;
    let request = window
      .data
      .runtime
      .spawn(async move { peer.call_tool(params).await.expect("tool answers") });
    while !request.is_finished() {
      window.tree.drain_mcp_requests(&mut window.app);
      window.tree.tick_futures();
      window.pass();
      thread::sleep(Duration::from_millis(5));
    }
    let result = window.data.runtime.block_on(request).expect("call finished");
    let result: Value = serde_json::to_value(result).expect("tool result");
    assert!(
      !result["isError"].as_bool().unwrap_or(false),
      "the tree is read: {result}"
    );
    result.to_string()
  }
}

#[test]
fn team_hub_profile_fix_automation_cannot_set_a_path_or_save_a_value() {
  let (mut agents, _, lint) = needing_path_and_value();
  let program = agents.root.join("lint-mcp");
  std::fs::write(&program, "#!/bin/sh\n").expect("a program");
  agents.stand_ins.choose(Some(program));
  let set_path = format!("team-hub-ready-mcp-{lint}-set-path");
  with_synthetic_input(|| agents.click(&set_path));
  assert!(agents.refused().is_some(), "refused to the client that pressed it");
  for _ in 0..20 {
    agents.window.tree.tick_futures();
    agents.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
  assert_eq!(
    agents.text(&format!("team-hub-ready-mcp-{lint}-words")),
    "Set its program path here",
    "automation set no path"
  );

  // A person opens the dialog and types; automation presses Save value.
  agents.press_until("team-hub-ready-secret-GITHUB_TOKEN-add-value", "the dialog", |s| {
    s.exists("team-hub-secret-dialog")
  });
  agents.set("team-hub-secret-value", VALUE);
  with_synthetic_input(|| agents.click("team-hub-secret-save"));
  assert!(agents.refused().is_some(), "refused to the client that pressed it");
  assert!(agents.exists("team-hub-secret-dialog"), "the dialog stays");
  assert_eq!(agents.value("team-hub-secret-value"), VALUE, "with what was typed");
  assert!(!kept(&agents), "automation kept no value");
}

#[test]
fn team_hub_profile_fix_automation_cannot_switch_a_server_on() {
  let mut agents = Agents::new(true);
  team(&agents);
  claude(&agents.root, "Claude 2", "2.1.173");
  let ((profile, _), lint) = builder_using(&agents, true);
  let lint = lint.expect("lint");
  ProjectDb::open(&agents.root)
    .expect("database")
    .set_team_mcp_on_here(lint, false)
    .expect("off here");
  show(&mut agents, profile);
  with_synthetic_input(|| agents.click(&format!("team-hub-ready-mcp-{lint}-switch-on")));
  assert!(agents.refused().is_some(), "refused to the client that pressed it");
  for _ in 0..20 {
    agents.window.tree.tick_futures();
    agents.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
  let switch = ProjectDb::open(&agents.root)
    .expect("database")
    .team_mcp_switch(lint)
    .expect("read");
  assert_eq!(switch, Some(false), "automation switched nothing on");
  // The person's own press does.
  agents.click(&format!("team-hub-ready-mcp-{lint}-switch-on"));
  agents.until("on here", |s| {
    ProjectDb::open(&s.root)
      .expect("database")
      .team_mcp_switch(lint)
      .expect("read")
      == Some(true)
  });
}

#[test]
fn team_hub_profile_fix_set_path_asks_the_picker_and_keeps_what_it_gives() {
  let (mut agents, _, lint) = needing_path_and_value();
  let set_path = format!("team-hub-ready-mcp-{lint}-set-path");
  // Closed without a choice: nothing changes.
  agents.stand_ins.choose(None);
  agents.click(&set_path);
  for _ in 0..20 {
    agents.window.tree.tick_futures();
    agents.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
  assert_eq!(
    agents.text(&format!("team-hub-ready-mcp-{lint}-words")),
    "Set its program path here"
  );
  assert!(!agents.exists("team-hub-fix-error"));
  let program = agents.root.join("lint-mcp");
  std::fs::write(&program, "#!/bin/sh\n").expect("a program");
  agents.stand_ins.choose(Some(program));
  agents.click(&set_path);
  agents.until("the path is kept", |s| {
    let words = format!("team-hub-ready-mcp-{lint}-words");
    let said = s.text(&words);
    let error = s.exists("team-hub-fix-error").then(|| s.text("team-hub-fix-error"));
    assert!(error.is_none(), "{error:?}");
    said == "Ready · program path set"
  });
}

#[test]
fn team_hub_profile_fix_a_blank_value_is_not_saved() {
  let (mut agents, ..) = needing_path_and_value();
  agents.press_until("team-hub-ready-secret-GITHUB_TOKEN-add-value", "the dialog", |s| {
    s.exists("team-hub-secret-dialog")
  });
  for typed in ["", "   "] {
    agents.set("team-hub-secret-value", typed);
    agents.click("team-hub-secret-save");
    for _ in 0..10 {
      agents.window.tree.tick_futures();
      agents.window.pass();
    }
    assert!(agents.exists("team-hub-secret-dialog"), "{typed:?} keeps the dialog");
    assert!(!kept(&agents), "{typed:?} keeps nothing");
  }
}

#[test]
fn team_hub_profile_fix_the_value_stays_out_of_the_tree_and_lands_in_the_keychain_by_name() {
  let (mut agents, ..) = needing_path_and_value();
  let client = Client::new(&mut agents);
  agents.press_until("team-hub-ready-secret-GITHUB_TOKEN-add-value", "the dialog", |s| {
    s.exists("team-hub-secret-dialog")
  });
  agents.set("team-hub-secret-value", VALUE);
  assert!(
    !client.tree(&mut agents).contains(VALUE),
    "the typed value never reaches the MCP tree"
  );
  agents.click("team-hub-secret-save");
  agents.until("the value is kept and the dialog is closed", |s| {
    kept(s) && !s.exists("team-hub-secret-dialog")
  });
  let services = agents.window.launch.services().expect("services");
  let stored = ProjectDb::open(&agents.root)
    .expect("database")
    .team_secret_value(services.vault.get(), &token())
    .expect("read")
    .expect("a value under the name");
  assert_eq!(stored.expose(), VALUE);
  assert!(
    !format!("{stored:?}").contains(VALUE),
    "the value redacts itself in Debug"
  );
  assert!(!client.tree(&mut agents).contains(VALUE), "nor after it is kept");
}

#[test]
fn team_hub_profile_fix_a_failed_save_stays_in_the_dialog_with_the_text_kept() {
  let (mut agents, ..) = needing_path_and_value();
  agents.press_until("team-hub-ready-secret-GITHUB_TOKEN-add-value", "the dialog", |s| {
    s.exists("team-hub-secret-dialog")
  });
  agents.set("team-hub-secret-value", VALUE);
  // The project stops being the team's behind the panel's back, so the workspace refuses the value.
  rusqlite::Connection::open(agents.root.join(".orchester/orchester.db"))
    .expect("fixture db")
    .execute_batch("DELETE FROM sharing")
    .expect("unlinked");
  agents.click("team-hub-secret-save");
  agents.until("the error in the dialog", |s| s.exists("team-hub-secret-error"));
  assert!(agents.exists("team-hub-secret-dialog"), "the dialog stays open");
  assert_eq!(
    agents.value("team-hub-secret-value"),
    VALUE,
    "the typed value is not lost"
  );
  assert!(!agents.text("team-hub-secret-error").contains(VALUE));
  assert!(!kept(&agents));
  // Cancel forgets it.
  agents.click("team-hub-secret-dialog-cancel");
  agents.until("closed", |s| !s.exists("team-hub-secret-dialog"));
  agents.press_until("team-hub-ready-secret-GITHUB_TOKEN-add-value", "the dialog", |s| {
    s.exists("team-hub-secret-dialog")
  });
  assert_eq!(
    agents.value("team-hub-secret-value"),
    "",
    "nothing is left of what was typed"
  );
  assert!(!agents.exists("team-hub-secret-error"), "nor of the failure");
}

#[test]
fn team_hub_profile_fix_a_panel_belongs_to_its_profile() {
  let (mut agents, profile, _) = needing_path_and_value();
  let other = other_profile(&agents);
  // The hub reads the new profile.
  agents
    .window
    .projects()
    .feed(super::support::project_id())
    .outside
    .update(|counts| counts.profiles += 1);
  agents.until("the new profile is listed", |s| {
    s.exists(&format!("team-hub-row-{other}"))
  });
  agents.press_until("team-hub-ready-secret-GITHUB_TOKEN-add-value", "the dialog", |s| {
    s.exists("team-hub-secret-dialog")
  });
  agents.set("team-hub-secret-value", VALUE);
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(other)));
  agents.until("the other profile", |s| {
    s.exists("team-hub-panel") && s.text("team-hub-panel-title") == "Other"
  });
  assert!(!agents.exists("team-hub-secret-dialog"), "no dialog carries over");
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the first profile", |s| {
    s.exists("team-hub-ready-secret-GITHUB_TOKEN-add-value")
  });
  assert!(!agents.exists("team-hub-secret-dialog"), "nor comes back");
  agents.press_until("team-hub-ready-secret-GITHUB_TOKEN-add-value", "the dialog", |s| {
    s.exists("team-hub-secret-dialog")
  });
  assert_eq!(
    agents.value("team-hub-secret-value"),
    "",
    "the typed value did not stay"
  );
}

/// A second team profile on Claude Code, which uses nothing.
fn other_profile(agents: &Agents) -> ResourceId {
  use std::collections::BTreeMap;

  use orchester_core::{
    connection::AgentProvider,
    id::ConnectionId,
    name::Name,
    resource::{AgentProfile, ModelId, PromptText, ProviderSettings, TeamAgentProfile},
  };

  use super::team_hub::{cached, keep_team};
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let team = TeamAgentProfile::new(
    Name::new("Other").expect("name"),
    String::new(),
    PromptText::new("Do other work.").expect("prompt"),
    (
      AgentProvider::ClaudeCodeCli,
      ModelId::new("claude-opus-5-5").expect("model"),
      ProviderSettings::new(BTreeMap::new()).expect("settings"),
    ),
  );
  let local = AgentProfile::from_team(&team, ConnectionId::generate());
  let other = cached(
    &mut db,
    ("team-other", "Other"),
    serde_json::to_value(&team).expect("json"),
    &local,
  );
  keep_team(agents);
  other
}

#[test]
fn team_hub_profile_fix_choosing_another_connection_after_a_pick_keeps_it() {
  let mut agents = Agents::new(true);
  team(&agents);
  let first = claude(&agents.root, "Claude 2", "2.1.173");
  let home = claude(&agents.root, "Claude Home", "2.1.170");
  let ((profile, _), _) = builder_using(&agents, false);
  show(&mut agents, profile);
  let pick = |agents: &Agents| {
    ProjectDb::open(&agents.root)
      .expect("database")
      .connection_pick(profile)
      .expect("pick")
      .map(|pick| pick.connection)
  };
  // Before a pick, the select only chooses; Use keeps it.
  choose(&mut agents, "Claude Code · Claude Home");
  for _ in 0..10 {
    agents.window.tree.tick_futures();
    agents.window.pass();
  }
  assert_eq!(
    pick(&agents),
    None,
    "choosing alone keeps nothing before the first pick"
  );
  assert!(agents.text("team-hub-fix-use").contains("Use Claude Home for Builder"));
  agents.click("team-hub-fix-use");
  agents.until("kept", |s| pick(s) == Some(home));
  agents.until("read back", |s| {
    s.text("team-hub-runs-on-sub") == "picked on this computer"
  });
  // After it, the select is the pick: choosing another keeps that one.
  choose(&mut agents, "Claude Code · Claude 2");
  agents.until("the other is kept", |s| pick(s) == Some(first));
  agents.until("read back", |s| {
    s.text("team-hub-ready-connection-words").contains("Claude 2")
  });
}

fn choose(agents: &mut Agents, label: &str) {
  let mut select = agents
    .window
    .tree
    .get_element_by_id_mut("team-hub-pick")
    .and_then(|e| e.as_select())
    .expect("the select");
  let index = select
    .labels()
    .iter()
    .position(|option| &**option == label)
    .expect("option");
  select.commit(index);
  agents.window.pass();
}

#[test]
fn team_hub_profile_fix_buttons_wait_while_the_hub_is_read_again() {
  let (mut agents, _, lint) = needing_path_and_value();
  let hold = agents.window.launch.services().expect("services").hold.clone();
  hold.set(true);
  agents
    .window
    .projects()
    .feed(super::support::project_id())
    .outside
    .update(|counts| counts.profiles += 1);
  // The hub is read again from the next frame, and the read waits for the hold.
  for _ in 0..5 {
    agents.window.tree.tick_futures();
    agents.window.pass();
  }
  let program = agents.root.join("lint-mcp");
  std::fs::write(&program, "#!/bin/sh\n").expect("a program");
  agents.stand_ins.choose(Some(program));
  agents.click(&format!("team-hub-ready-mcp-{lint}-set-path"));
  agents.click("team-hub-ready-secret-GITHUB_TOKEN-add-value");
  for _ in 0..20 {
    agents.window.tree.tick_futures();
    agents.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
  assert!(
    !agents.exists("team-hub-secret-dialog"),
    "the link does nothing while reading"
  );
  hold.set(false);
  for _ in 0..20 {
    agents.window.tree.tick_futures();
    agents.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
  // Not read again by the press that came meanwhile: nothing was set.
  assert_eq!(
    agents.text(&format!("team-hub-ready-mcp-{lint}-words")),
    "Set its program path here"
  );
}

/// A pick made outside the panel (an MCP client's `pick_profile_connection`) shows in the select without reopening it
/// (ORC-848 D5), and choosing in the select afterwards keeps that one as a pick made in the panel does.
#[test]
fn team_hub_profile_fix_the_select_follows_a_pick_made_elsewhere() {
  let mut agents = Agents::new(true);
  team(&agents);
  let first = claude(&agents.root, "Claude 2", "2.1.173");
  let home = claude(&agents.root, "Claude Home", "2.1.170");
  let ((profile, _), _) = builder_using(&agents, false);
  show(&mut agents, profile);
  assert!(
    agents.text("team-hub-pick").contains("Choose a Claude Code connection"),
    "{}",
    agents.text("team-hub-pick")
  );

  let workspace = agents.window.launch.workspace().expect("workspace");
  workspace
    .pick_team_connection(
      &agents.root,
      profile,
      ConnectionPick {
        connection: home,
        account: None,
      },
    )
    .expect("a pick made by an MCP client");

  agents.until("the select follows", |s| {
    s.text("team-hub-pick").contains("Claude Home")
  });
  assert!(!agents.text("team-hub-pick").contains("Choose a Claude Code connection"));
  choose(&mut agents, "Claude Code · Claude 2");
  agents.until("the other is kept", |s| {
    ProjectDb::open(&s.root)
      .expect("database")
      .connection_pick(profile)
      .expect("pick")
      .is_some_and(|pick| pick.connection == first)
  });
}
