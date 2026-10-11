//! The window editor's authoritative MCP selection check.
use orchester_core::{
  connection::mcp::{McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
  resource::CommandProgram,
  store::McpServerStore,
};
use orchester_workspace::agents::{
  data::ProfileEntry,
  editor::{self, Action, Outcome, Request},
};

use super::*;

#[test]
fn profile_editor_checks_enabled_added_disabled_kept_removed_and_missing_servers() {
  let (repo, mut db) = project_db();
  let connection = codex(&repo);
  db.create_connection(&connection).expect("connection");
  let original = form(&connection, &ModelSelection::Listed(model("gpt-5.5")), None)
    .profile()
    .expect("profile");
  let mut server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("test-mcp").expect("name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("test-mcp").expect("program"),
      vec![],
      None,
    )),
    McpEnvironment::default(),
    now(),
  )
  .expect("server");
  db.create_mcp_server(&server).expect("store server");
  let selected = original
    .clone()
    .with_mcp_servers(McpServerSelection::new(vec![server.id()]).expect("selection"));
  let id = create_profile(&mut db, &original, now()).expect("create");
  let entry = |db: &ProjectDb| -> ProfileEntry {
    ProjectAgents::load(db)
      .expect("load")
      .profile(id)
      .expect("profile")
      .clone()
  };
  let request = |existing: ProfileEntry, profile: AgentProfile| Request {
    existing: Some(existing),
    profile,
    action: Action::Save,
    allowed: vec![],
  };
  let mut enabled = request(entry(&db), selected.clone());
  let Outcome::Confirm(reasons) = editor::apply(repo.path(), &enabled).expect("offered") else {
    panic!("adding a server asks the person")
  };
  enabled.allowed = reasons;
  assert_eq!(
    editor::apply(repo.path(), &enabled).expect("confirmed"),
    Outcome::Saved(id)
  );
  server.set_enabled(false);
  db.save_mcp_server(&server).expect("disable");
  let kept = request(entry(&db), selected.clone());
  assert!(
    editor::apply(repo.path(), &kept).is_ok(),
    "kept disabled selection saves"
  );
  let removed = request(entry(&db), original.clone());
  assert_eq!(
    editor::apply(repo.path(), &removed).expect("remove"),
    Outcome::Saved(id)
  );
  let added_disabled = request(entry(&db), selected);
  let error = editor::apply(repo.path(), &added_disabled).expect_err("new disabled choice");
  assert_eq!(error.key, "appMcp.errors.mcpServerOff");
  assert_eq!(error.args, [("server", "test-mcp".to_string())]);
  assert_eq!(entry(&db).profile, original);
  let missing = ConnectionId::generate();
  let unknown = request(
    entry(&db),
    original
      .clone()
      .with_mcp_servers(McpServerSelection::new(vec![missing]).expect("selection")),
  );
  let error = editor::apply(repo.path(), &unknown).expect_err("missing server");
  assert_eq!(error.key, "appMcp.errors.mcpServerMissing");
  assert_eq!(error.args, [("server", missing.to_string())]);
  assert_eq!(entry(&db).profile, original);
}

#[test]
fn profile_editor_permission_failure_preserves_option_detail_and_storage() {
  for (provider, commands, key) in [
    (
      AgentProvider::CodexCli,
      CommandPermission::Allowed(AllowedCommands::default()),
      "agents.permissions.notEnforceable.allowedCommands",
    ),
    (
      AgentProvider::ClaudeCodeCli,
      CommandPermission::AnyInWorktree,
      "agents.permissions.notEnforceable.anyCommand",
    ),
  ] {
    let (repo, mut db) = project_db();
    let connection = AgentConnection::new(
      ConnectionId::generate(),
      Name::new("Test CLI").expect("name"),
      provider,
      repo.path().join("agent-cli"),
      now(),
    )
    .expect("connection");
    db.create_connection(&connection).expect("store connection");
    let original = form(&connection, &ModelSelection::Custom, None);
    let original = ProfileForm {
      custom_model: "test-model",
      ..original
    }
    .profile()
    .expect("profile");
    let id = create_profile(&mut db, &original, now()).expect("create");
    let existing = ProjectAgents::load(&db)
      .expect("load")
      .profile(id)
      .expect("entry")
      .clone();
    let candidate = original.clone().with_permissions(AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      commands,
      NetworkAccess::On,
    ));
    let error = editor::apply(
      repo.path(),
      &Request {
        existing: Some(existing.clone()),
        profile: candidate,
        action: Action::Save,
        allowed: vec![],
      },
    )
    .expect_err("unsupported option");
    assert_eq!(error.key, key);
    assert_eq!(ProjectAgents::load(&db).expect("load").profile(id), Some(&existing));
  }
}
