use super::*;

#[test]
fn chat_mcp_add_is_a_card_with_the_whole_server_that_only_the_person_applies() {
  let desktop = Desktop::new();
  // Not even while the person lets automation widen: adding a server is always the person's.
  desktop.let_automation_change_permissions(true);
  let told = desktop
    .chat(&desktop.scope, "mcp_add_server", pencil())
    .expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  assert_eq!(told["proposed"]["mcp_server"]["operation"], "add", "{told}");
  assert!(desktop.servers().is_empty(), "nothing is added on the agent's word");
  let (seq, action) = desktop.last_action();
  assert_eq!(action.outcome, ActionOutcome::Waits);
  let mcp = mcp_of(&action);
  assert_eq!(mcp.operation, McpOperation::Add);
  assert!(mcp.widens && mcp.id.is_none(), "{mcp:?}");
  let server = &mcp.server;
  assert_eq!(server.name, "pencil");
  assert!(
    matches!(
      &server.transport,
      McpTransportShown::Stdio { program: shown, found: Some(found), args, working_directory: None }
        if *shown == program() && *found == program() && args == &["--app", "desktop"]
    ),
    "{:?}",
    server.transport
  );
  assert_eq!(
    server.environment,
    [
      McpVariableShown {
        name: "PENCIL_KEY".to_owned(),
        secret: Some("PENCIL_TOKEN@development".to_owned()),
      },
      McpVariableShown {
        name: "RUST_LOG".to_owned(),
        secret: None,
      },
    ],
    "variables by name, a secret by its name"
  );
  assert_eq!(server.exempt_tools, ["get_screenshot"]);
  assert!(server.enabled && server.used_by.is_empty());

  desktop.apply(seq);
  assert_eq!(desktop.last_action().1.outcome, ActionOutcome::Applied);
  assert_eq!(
    desktop.servers(),
    [("pencil".to_owned(), true)],
    "the person's Apply added it"
  );
}

#[test]
fn chat_mcp_http_server_names_its_authorization_secret_and_refuses_one_the_project_has_not() {
  let desktop = Desktop::new();
  let remote = |authorization: Value| {
    json!({
      "project": "Kontur",
      "name": "remote",
      "url": format!("https://mcp.example.com/mcp?token={QUERY_TOKEN}"),
      "authorization": authorization,
    })
  };
  let schemes = [
    (
      json!({ "scheme": "bearer", "secret": "PENCIL_TOKEN@development" }),
      McpAuthorizationShown::Bearer {
        secret: "PENCIL_TOKEN@development".to_owned(),
      },
    ),
    (
      json!({ "scheme": "basic", "username": "ci", "secret": "PENCIL_TOKEN@development" }),
      McpAuthorizationShown::Basic {
        username: "ci".to_owned(),
        secret: "PENCIL_TOKEN@development".to_owned(),
      },
    ),
    (
      json!({ "scheme": "basic_credential", "secret": "PENCIL_TOKEN@development" }),
      McpAuthorizationShown::BasicCredential {
        secret: "PENCIL_TOKEN@development".to_owned(),
      },
    ),
    (
      json!({ "scheme": "header", "name": "X-API-Key", "secret": "PENCIL_TOKEN@development" }),
      McpAuthorizationShown::Header {
        name: "X-API-Key".to_owned(),
        secret: "PENCIL_TOKEN@development".to_owned(),
      },
    ),
  ];
  for (authorization, shown) in schemes {
    let told = desktop
      .chat(&desktop.scope, "mcp_add_server", remote(authorization))
      .expect("proposed");
    let (_, action) = desktop.last_action();
    let mcp = mcp_of(&action);
    assert_eq!(
      mcp.server.transport,
      McpTransportShown::Http {
        url: "https://mcp.example.com/mcp?token=…".to_owned(),
        authorization: shown,
      }
    );
    assert!(
      !told.to_string().contains(QUERY_TOKEN),
      "a query value is not told: {told}"
    );
  }
  let unknown = desktop
    .chat(
      &desktop.scope,
      "mcp_add_server",
      remote(json!({ "scheme": "bearer", "secret": "NOT_THERE@development" })),
    )
    .expect_err("only an active project secret can be named");
  assert!(
    matches!(&unknown, ToolError::Refused { code, .. } if code == "invalid_server"),
    "{unknown:?}"
  );
  assert_eq!(
    refusal(&desktop.last_action().1),
    "invalid_server",
    "the refusal is kept"
  );
  assert!(desktop.servers().is_empty());
}

#[test]
fn chat_mcp_card_never_holds_a_plain_variable_value_or_an_endpoints_query() {
  let desktop = Desktop::new();
  desktop.let_automation_change_permissions(true);
  desktop.app(
    "add_mcp_server",
    json!({
      "project": "Kontur",
      "name": "pencil",
      "program": program(),
      "environment": [{ "name": "API_KEY", "value": PASTED }],
    }),
  );
  desktop.app(
    "add_mcp_server",
    json!({
      "project": "Kontur",
      "name": "remote",
      "url": format!("https://mcp.example.com/mcp?token={QUERY_TOKEN}"),
    }),
  );
  for server in ["pencil", "remote"] {
    let told = desktop
      .chat(
        &desktop.scope,
        "mcp_update_server",
        json!({ "project": "Kontur", "server": server, "exempt_tools": ["read"] }),
      )
      .expect("proposed");
    let (_, action) = desktop.last_action();
    let kept = serde_json::to_string(&action).expect("the action reads as JSON");
    for hidden in [PASTED, QUERY_TOKEN] {
      assert!(!kept.contains(hidden), "the card keeps no {hidden}: {kept}");
      assert!(
        !told.to_string().contains(hidden),
        "the agent is not told {hidden}: {told}"
      );
    }
    assert!(mcp_of(&action).widens, "a tool that runs without asking widens");
  }
  let (_, action) = desktop.last_action();
  assert_eq!(
    mcp_of(&action).basis.len(),
    64,
    "a digest of the stored server stands in for it"
  );
}

#[test]
fn chat_mcp_card_is_refused_once_the_server_changed_since() {
  let desktop = Desktop::new();
  desktop.let_automation_change_permissions(true);
  desktop.app(
    "add_mcp_server",
    json!({ "project": "Kontur", "name": "pencil", "program": program(), "args": ["--app", "desktop"] }),
  );
  desktop
    .chat(
      &desktop.scope,
      "mcp_update_server",
      json!({ "project": "Kontur", "server": "pencil", "exempt_tools": ["get_screenshot"] }),
    )
    .expect("proposed");
  let (seq, _) = desktop.last_action();
  // Meanwhile someone changed the server's arguments in Connections.
  desktop.app(
    "update_mcp_server",
    json!({ "project": "Kontur", "server": "pencil", "args": ["--app", "web"] }),
  );
  desktop.apply(seq);
  let (_, action) = desktop.last_action();
  assert_eq!(refusal(&action), "changed_since_proposed");
  let stored = desktop.db().mcp_servers().expect("servers").remove(0);
  assert!(
    stored.exempt_tools().is_empty(),
    "the card's change was not made over what the person was not shown"
  );

  // A program not found is a warning on the card; applying it is refused, nothing added.
  let missing = desktop
    .repo
    .path()
    .parent()
    .expect("a folder")
    .join("no-such-pencil.exe");
  desktop
    .chat(
      &desktop.scope,
      "mcp_add_server",
      json!({ "project": "Kontur", "name": "missing", "program": missing.to_string_lossy() }),
    )
    .expect("proposed with a warning");
  let (seq, action) = desktop.last_action();
  assert!(
    matches!(
      &mcp_of(&action).server.transport,
      McpTransportShown::Stdio { found: None, .. }
    ),
    "the card warns the program is not found"
  );
  desktop.apply(seq);
  assert_eq!(refusal(&desktop.last_action().1), "program_not_found");
  assert_eq!(desktop.servers().len(), 1, "nothing was added");
}

#[test]
fn chat_mcp_narrowing_is_made_at_once_only_under_the_persons_switch_in_a_turn_they_asked_for() {
  let desktop = Desktop::new();
  desktop.let_automation_change_permissions(true);
  desktop.app(
    "add_mcp_server",
    json!({ "project": "Kontur", "name": "pencil", "program": program() }),
  );
  desktop.let_automation_change_permissions(false);
  let off = json!({ "project": "Kontur", "server": "pencil", "enabled": false });
  let told = desktop
    .chat(&desktop.scope, "mcp_set_server_enabled", off.clone())
    .expect("proposed");
  assert_eq!(told["waits_for_person"], true, "a card while the switch is off: {told}");
  assert_eq!(desktop.servers(), [("pencil".to_owned(), true)]);
  let (seq, action) = desktop.last_action();
  assert!(!mcp_of(&action).widens);
  desktop.apply(seq);
  assert_eq!(
    desktop.servers(),
    [("pencil".to_owned(), false)],
    "the person turned it off"
  );

  desktop.let_automation_change_permissions(true);
  let on_its_own = ChatScope {
    on_its_own: true,
    ..desktop.scope.clone()
  };
  let on = json!({ "project": "Kontur", "server": "pencil", "enabled": true });
  desktop
    .chat(&on_its_own, "mcp_set_server_enabled", on)
    .expect("proposed");
  assert_eq!(
    desktop.last_action().1.outcome,
    ActionOutcome::Waits,
    "turning on widens"
  );
  assert_eq!(desktop.servers(), [("pencil".to_owned(), false)]);

  desktop.apply(desktop.last_action().0);
  assert_eq!(desktop.servers(), [("pencil".to_owned(), true)]);
  desktop
    .chat(&on_its_own, "mcp_set_server_enabled", off.clone())
    .expect("proposed");
  assert_eq!(
    desktop.last_action().1.outcome,
    ActionOutcome::Waits,
    "a turn nobody asked for only proposes"
  );
  desktop
    .chat(&desktop.scope, "mcp_set_server_enabled", off)
    .expect("turned off");
  assert_eq!(desktop.last_action().1.outcome, ActionOutcome::Done, "made at once");
  assert_eq!(desktop.servers(), [("pencil".to_owned(), false)]);

  let removed = desktop
    .chat(
      &desktop.scope,
      "mcp_remove_server",
      json!({ "project": "Kontur", "server": "pencil" }),
    )
    .expect("removed");
  assert_eq!(removed["removed"]["name"], "pencil", "{removed}");
  let (_, action) = desktop.last_action();
  assert_eq!(action.outcome, ActionOutcome::Done);
  assert_eq!(mcp_of(&action).operation, McpOperation::Remove);
  assert!(desktop.servers().is_empty());
}

#[test]
fn chat_mcp_card_the_person_dismisses_changes_nothing() {
  let desktop = Desktop::new();
  desktop
    .chat(&desktop.scope, "mcp_add_server", pencil())
    .expect("proposed");
  let (seq, _) = desktop.last_action();
  desktop.decide(seq, Decision::Dismiss);
  assert_eq!(desktop.last_action().1.outcome, ActionOutcome::Dismissed);
  assert!(desktop.servers().is_empty());
}
