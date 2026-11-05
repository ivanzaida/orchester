//! Regression cases returned by ORC-2 review.
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, EndpointPreset, invocation::Platform, mcp::McpServerSelection,
  },
  id::{ConnectionId, ResourceId, SecretId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::{AgentProfile, Draft},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::agents::data::{self, ProfileChange};

use super::harness::Agents;

fn save_permissions(screen: &Agents, id: ResourceId, permissions: AgentPermissions) {
  let profile = screen
    .listed()
    .profile(id)
    .expect("profile")
    .profile
    .clone()
    .with_permissions(permissions);
  data::apply_in(&screen.root, ProfileChange::SaveDraft(id, profile)).expect("fixture draft");
}

#[test]
fn agents_network_summary_reports_effective_access_in_list_header_and_panel() {
  let mut screen = Agents::new(true);
  let id = screen.profile(false);
  let permissions = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  );
  let fallback = permissions.effective_network(AgentProvider::CodexCli, Platform::current()) != NetworkAccess::Off;
  save_permissions(&screen, id, permissions);
  screen.go();
  let expected = if fallback {
    "Network on · cannot block"
  } else {
    "Network off"
  };
  assert!(screen.text(&format!("agent-{id}")).contains(expected));
  screen.click(&format!("agent-{id}"));
  screen.until("the profile's editor", |s| s.exists("agent-permission-summary"));
  assert!(screen.text("agent-permission-summary").contains(expected));
  if fallback {
    assert!(!screen.text("agent-permission-summary").contains("Enforced by"));
    assert!(screen.text("agent-permission-summary").contains("Network stays on"));
  }
  // The enforcement panel (01c) says, for Network, what enforces it or that nothing can.
  let network = screen.text("agent-enforcement-network");
  assert!(network.contains("Network"), "{network}");
  if fallback {
    assert!(!network.contains("Enforced by"), "{network}");
    assert!(screen.exists("agent-network-reason"));
  }
}

#[test]
fn agents_selected_allowed_commands_keeps_prefixes_through_save() {
  let mut screen = Agents::new(false);
  let mut db = ProjectDb::open(&screen.root).expect("db");
  let claude = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude CLI").expect("name"),
    AgentProvider::ClaudeCodeCli,
    screen.root.join("claude"),
    Timestamp::now(),
  )
  .expect("connection");
  db.create_connection(&claude).expect("store connection");
  let id = screen.profile(false);
  let prefixes = AllowedCommands::new(vec![CommandPrefix::new("cargo test").expect("prefix")]).expect("list");
  save_permissions(
    &screen,
    id,
    AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::Allowed(prefixes.clone()),
      NetworkAccess::On,
    ),
  );
  data::publish(&mut db, id, Timestamp::now()).expect("published prefix list");
  screen.open(id);
  // Manual shows its allowed commands under it (07).
  assert!(screen.text("agent-commands").contains("cargo test"));
  screen.set("agent-responsibility", "Keep command prefixes");
  screen.until("saved on its own", |s| {
    s.listed().profile(id).expect("profile").profile.responsibilities() == "Keep command prefixes"
  });
  assert_eq!(
    screen
      .listed()
      .profile(id)
      .expect("profile")
      .profile
      .permissions()
      .commands(),
    &CommandPermission::Allowed(prefixes)
  );
}

#[test]
fn agents_cli_to_native_transition_can_save_without_hidden_cli_values() {
  let mut screen = Agents::new(true);
  let id = screen.profile(false);
  let mut db = ProjectDb::open(&screen.root).expect("db");
  let native = AgentConnection::native(
    ConnectionId::generate(),
    Name::new("Anthropic API").expect("name"),
    AgentProvider::AnthropicApi,
    ApiEndpoint::new(
      EndpointPreset::Official,
      BaseUrl::new("https://api.anthropic.com").expect("url"),
      SecretId::generate(),
    ),
    Timestamp::now(),
  )
  .expect("native");
  db.create_connection(&native).expect("store native");
  let mut profile = screen.listed().profile(id).expect("profile").profile.clone();
  let mut permissions = profile.permissions().clone();
  permissions.set_interactive_tool_approvals(true);
  profile = profile
    .with_permissions(permissions)
    .with_mcp_servers(McpServerSelection::new(vec![ConnectionId::generate()]).expect("old selection"));
  data::apply_in(&screen.root, ProfileChange::SaveDraft(id, profile)).expect("fixture");
  screen.open(id);
  screen.click("agent-connection");
  let option = (0..8)
    .map(|index| format!("agent-connection-option-{index}"))
    .find(|id| screen.exists(id) && screen.text(&format!("{id}-label")).contains("Anthropic API"))
    .expect("API option");
  screen.click(&option);
  screen.set("agent-custom-model", "claude-test");
  assert!(screen.text("agent-mcp").contains("does not offer approval switches"));
  // A native connection keeps Manual's allowed commands, without the MCP approvals switch.
  assert!(screen.exists("agent-commands") && !screen.exists("agent-ask-tools"));
  screen.until("saved native on its own", |s| {
    s.listed().profile(id).expect("profile").profile.connection() == native.id()
  });
  let saved = screen.listed().profile(id).expect("profile").profile.clone();
  assert_eq!(saved.connection(), native.id());
  assert!(!saved.permissions().interactive_tool_approvals());
  assert!(saved.mcp_servers().ids().is_empty());
}

#[test]
fn agents_stale_draft_bar_keeps_its_actual_base_version() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  let mut db = ProjectDb::open(&screen.root).expect("db");
  let published = db.latest_version::<AgentProfile>(id).expect("read").expect("v1");
  data::save_draft(&mut db, id, super::harness::changed(published.body()), Timestamp::now()).expect("draft");
  data::publish(&mut db, id, Timestamp::now()).expect("v2");
  db.save_draft(
    id,
    &Draft::new(
      super::harness::changed(published.body()),
      Some(published.number()),
      Timestamp::now(),
    ),
  )
  .expect("stale");
  screen.open(id);
  let bar = screen.text("agent-draft-bar");
  assert!(bar.contains("Draft based on v1"), "{bar}");
  assert!(bar.contains("Published v2"), "{bar}");
  assert!(bar.contains("Publishing v3 replaces v2’s changes"), "{bar}");
}

#[test]
fn agents_editing_while_a_save_is_under_way_stays_editable_and_saves_next() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.set("agent-responsibility", "Candidate being saved");
  screen.until("saved on its own", |s| {
    s.listed().profile(id).expect("profile").profile.responsibilities() == "Candidate being saved"
  });
  // The fields stay on the page while it saves: an edit goes on and saves after.
  assert!(screen.exists("agent-responsibility") && screen.exists("agent-option-EditWorktree"));
  screen.set("agent-responsibility", "Next candidate");
  screen.until("the next edit saved", |s| {
    s.listed().profile(id).expect("profile").profile.responsibilities() == "Next candidate"
  });
}

#[test]
fn agents_a_late_confirmation_cannot_reinstate_an_old_candidate() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  screen.click("agent-option-EditWorktree");
  screen.until("question", |s| s.exists("agent-confirm-confirm"));
  screen.set("agent-responsibility", "New candidate after question");
  screen.until("new question", |s| s.exists("agent-confirm-confirm"));
  screen.window.tree.tick_futures();
  screen.window.pass();
  screen.click("agent-confirm-confirm");
  screen.until("confirmed", |s| {
    let saved = s.listed().profile(id).expect("profile").profile.clone();
    saved.responsibilities() == "New candidate after question"
      && saved.permissions().repository() == RepositoryAccess::Write
  });
}

#[test]
fn agents_failed_save_keeps_the_unsaved_fields() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  let entry = screen.listed().profile(id).expect("profile").clone();
  data::apply_in(
    &screen.root,
    ProfileChange::SaveDraft(id, super::harness::changed(&entry.profile)),
  )
  .expect("outside edit");
  screen.set("agent-responsibility", "Keep this unsaved value");
  screen.until("failed or told", |s| {
    (s.exists("agent-save-error") || s.exists("agent-outside-change")) && s.exists("agent-role")
  });
  assert_eq!(screen.value("agent-responsibility"), "Keep this unsaved value");
}

#[test]
fn agents_delayed_folder_add_preserves_removal_and_saved_permissions() {
  use orchester_core::permission::WritableFolders;
  use orchester_workspace::agents::folders;

  let mut screen = Agents::new(true);
  let id = screen.profile(false);
  let mut permissions = screen
    .listed()
    .profile(id)
    .expect("profile")
    .profile
    .permissions()
    .clone();
  permissions.set_repository(RepositoryAccess::Write);
  permissions.set_writable_folders(
    folders::with_folder(&WritableFolders::none(), ".orchester/build-a", &screen.root).expect("folder A"),
  );
  save_permissions(&screen, id, permissions);
  screen.open(id);
  screen.set("agent-folder", ".orchester/build-b");
  screen.click("agent-add-folder");
  // No tick_futures: validation cannot be delivered before the person removes A.
  screen.click("agent-folder-0-remove");
  assert!(
    !screen.text("agent-folders-row").contains(".orchester/build-a"),
    "{}",
    screen.text("agent-folders-row")
  );
  screen.until("folder B added", |s| {
    s.text("agent-folders-row").contains(".orchester/build-b") && s.exists("agent-folder-0-remove")
  });
  assert!(!screen.text("agent-folders-row").contains(".orchester/build-a"));

  screen.set("agent-folder", ".orchester/build-b");
  screen.click("agent-add-folder");
  screen.until("duplicate refused", |s| s.exists("agent-folder-note"));
  assert!(!screen.exists("agent-folder-1-remove"));
  // The folders saved on their own, after the widening question at the field.
  screen.until("saved", |s| {
    if s.exists("agent-confirm-confirm") {
      s.window.tree.tick_futures();
      s.window.pass();
      s.click("agent-confirm-confirm");
    }
    let saved = s.listed().profile(id).expect("profile").profile.clone();
    saved
      .permissions()
      .writable_folders()
      .entries()
      .iter()
      .map(|f| f.as_str())
      .eq([".orchester/build-b"])
  });
  let saved = screen.listed().profile(id).expect("profile").profile.clone();
  let folders: Vec<_> = saved
    .permissions()
    .writable_folders()
    .entries()
    .iter()
    .map(|folder| folder.as_str())
    .collect();
  assert_eq!(folders, [".orchester/build-b"]);
}

#[test]
fn agents_connection_menu_names_each_connections_cli_and_account() {
  let mut screen = Agents::new(true);
  let id = screen.profile(false);
  screen.open(id);
  screen.click("agent-connection");
  screen.until("the connections", |s| s.exists("agent-connection-option-0"));
  let entry = screen.text("agent-connection-option-0");
  assert!(entry.contains("account Default"), "{entry}");
  // What follows the comma goes on in lower case ("account Default, not checked yet · …").
  let (_, detail) = entry
    .split_once("account Default, ")
    .expect("the account's detail after a comma");
  assert!(detail.chars().next().is_some_and(char::is_lowercase), "{entry}");
  assert!(
    screen.exists("agent-connection-option-0-icon"),
    "each entry leads with its icon"
  );
}
