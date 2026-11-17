//! The Orchestrator chat changes a team project's MCP servers through the app's own MCP server tools, which change the
//! team's resources on the team server: no card is made of such a change, since a card reads this computer's servers,
//! and nothing is added to the project's own. A team server this computer cannot reach refuses it as those tools do.

use orchester_core::{
  id::{ProjectId, ServerId},
  project::ProjectKey,
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_store::SharingLink;

use super::*;

/// Links the project of `desktop` to a team server, as a member of it.
fn share(desktop: &Desktop) {
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: Timestamp::from_unix_millis(100),
    cursor: 0,
    last_synced_at: Timestamp::from_unix_millis(100),
    removed_at: None,
    address: None,
  };
  desktop
    .db()
    .start_sharing(&link, &[], (&[], &Default::default()))
    .expect("shared");
}

#[test]
fn chat_mcp_in_a_team_project_a_change_goes_to_the_apps_tool_and_no_card_is_made() {
  let desktop = Desktop::new();
  desktop.let_automation_change_permissions(true);
  share(&desktop);
  let server = json!({ "project": "Kontur", "server": "pencil" });
  let calls = [
    ("mcp_add_server", pencil()),
    (
      "mcp_update_server",
      json!({ "project": "Kontur", "server": "pencil", "args": ["--other"] }),
    ),
    (
      "mcp_set_server_enabled",
      json!({ "project": "Kontur", "server": "pencil", "enabled": true }),
    ),
    (
      "mcp_set_server_enabled",
      json!({ "project": "Kontur", "server": "pencil", "enabled": false }),
    ),
    ("mcp_remove_server", server),
  ];

  for (tool, arguments) in calls {
    // The project is linked to a team server nobody signed in to: the app's tool is reached, and refuses as it does.
    let refused = desktop.chat(&desktop.scope, tool, arguments).expect_err(tool);
    let refused = refused.to_json(|message| message.key.to_owned());
    assert_ne!(
      refused["error"]["code"], "team_mcp_proposal_unsupported",
      "{tool}: {refused}"
    );
  }
  assert!(desktop.servers().is_empty(), "nothing was added");
  let kept = desktop.db().chat_entries(desktop.scope.chat).expect("entries");
  assert!(
    kept
      .iter()
      .all(|entry| !matches!(entry.kind, EntryKind::Action(ref action) if action.change.is_some())),
    "no card was kept"
  );
}
