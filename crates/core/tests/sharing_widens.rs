//! Whether a teammate's version widens what this computer's agents may do (`docs/architecture/server.md`, row 35):
//! broader profile permissions or a new MCP server, a new secret, a tool's changed command, address, method, headers,
//! database, query or access policy, and a first version that grants any of them. Narrower, renamed and unchanged
//! versions apply at once. Run with `cargo test -p orchester-core --test sharing_widens sharing_widens`.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  connection::mcp::McpServerSelection,
  id::{ConnectionId, ResourceId, SecretId},
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    widening::{CommandWidening, Widening},
  },
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AllowedSteps, ApprovalMode, CommandAction, CommandArg,
    CommandEnvironment, CommandProgram, DatabaseQuery, DeniedBehavior, ExitCodes, FieldName, HttpAuth, HttpMethod,
    HttpRequest, ModelId, PromptText, ProviderSettings, RequestTarget, Tool, ToolAction, VariableName,
  },
  sharing::{VersionWidening, widens},
};

fn allowed(commands: &[&str]) -> CommandPermission {
  let prefixes = commands
    .iter()
    .map(|command| CommandPrefix::new(command).expect("a command prefix"))
    .collect();
  CommandPermission::Allowed(AllowedCommands::new(prefixes).expect("an allowed list"))
}

fn profile(role: &str, permissions: AgentPermissions, servers: &[ConnectionId]) -> AgentProfile {
  AgentProfile::new(
    Name::new(role).expect("valid role"),
    "Implements the approved plan.".into(),
    PromptText::new("You implement {{task.title}}.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("valid model"),
    ProviderSettings::new(BTreeMap::new()).expect("no settings"),
  )
  .with_permissions(permissions)
  .with_mcp_servers(McpServerSelection::new(servers.to_vec()).expect("a selection"))
}

fn read_only() -> AgentPermissions {
  AgentPermissions::new(RepositoryAccess::ReadOnly, allowed(&["cargo test"]), NetworkAccess::Off)
}

fn command(program: &str, args: &[&str], variables: &[(&str, &str)]) -> CommandAction {
  let args = args
    .iter()
    .map(|arg| CommandArg::new(arg).expect("valid argument"))
    .collect();
  let variables = variables
    .iter()
    .map(|(name, value)| (VariableName::new(name).expect("valid name"), (*value).to_owned()))
    .collect();
  CommandAction::new(CommandProgram::new(program).expect("valid program"), args)
    .with_environment(CommandEnvironment::new(variables).expect("valid variables"))
}

fn http(method: HttpMethod, url: &str, auth: HttpAuth, header: Option<&str>) -> HttpRequest {
  let inputs = header.map_or_else(BTreeMap::new, |name| {
    BTreeMap::from([(
      FieldName::new("token").expect("a field"),
      RequestTarget::Header(name.into()),
    )])
  });
  let host = url.trim_start_matches("https://").split('/').next().expect("a host");
  HttpRequest::new(method, url.into(), host.into(), auth, inputs, BTreeMap::new()).expect("a request")
}

fn policy(approval: ApprovalMode) -> AccessPolicy {
  AccessPolicy::new(
    BTreeMap::from([(ResourceId::generate(), AllowedSteps::All)]),
    Name::new("Staging").expect("valid environment"),
    approval,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("a policy")
}

fn tool(action: ToolAction, access: &AccessPolicy) -> Tool {
  Tool::new("Deploys a preview.", action, Some(access.clone())).expect("a tool")
}

#[test]
fn sharing_widens_nothing_for_the_same_a_renamed_or_a_narrower_profile() {
  let server = ConnectionId::generate();
  let mut wide = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  wide.set_auto_approve(true);
  let previous = profile("Builder", wide, &[server]);
  assert!(widens(Some(&previous), &previous).is_empty(), "the same version");
  let renamed = profile("Implementer", previous.permissions().clone(), &[server]);
  assert!(widens(Some(&previous), &renamed).is_empty(), "a renamed profile");
  let narrower = profile("Builder", read_only(), &[]);
  assert!(
    widens(Some(&previous), &narrower).is_empty(),
    "narrower permissions and a server dropped"
  );
}

#[test]
fn sharing_widens_names_broader_profile_permissions_and_a_new_mcp_server() {
  let kept = ConnectionId::generate();
  let previous = profile("Builder", read_only(), &[kept]);
  let broader = profile(
    "Builder",
    AgentPermissions::new(
      RepositoryAccess::Write,
      allowed(&["cargo test", "curl"]),
      NetworkAccess::On,
    ),
    &[kept],
  );
  assert_eq!(
    widens(Some(&previous), &broader),
    [
      VersionWidening::Permissions(Widening::Repository),
      VersionWidening::Permissions(Widening::Commands),
      VersionWidening::Permissions(Widening::Network),
    ]
  );
  let another_server = profile("Builder", read_only(), &[kept, ConnectionId::generate()]);
  assert_eq!(widens(Some(&previous), &another_server), [VersionWidening::McpServer]);
  let swapped = profile("Builder", read_only(), &[ConnectionId::generate()]);
  assert_eq!(
    widens(Some(&previous), &swapped),
    [VersionWidening::McpServer],
    "a server in place of another"
  );
}

#[test]
fn sharing_widens_a_first_profile_only_by_what_it_grants() {
  let nothing = profile(
    "Reviewer",
    AgentPermissions::new(RepositoryAccess::ReadOnly, allowed(&[]), NetworkAccess::Off),
    &[],
  );
  assert!(widens(None, &nothing).is_empty(), "a first version that grants nothing");
  let granting = profile("Builder", read_only(), &[ConnectionId::generate()]);
  assert_eq!(
    widens(None, &granting),
    [
      VersionWidening::Permissions(Widening::Commands),
      VersionWidening::McpServer
    ]
  );
}

#[test]
fn sharing_widens_a_command_tool_whose_program_arguments_or_environment_changed() {
  let previous = Tool::command("Runs the tests.", command("cargo", &["test"], &[("PATH", "C:/bin")]));
  let program = Tool::command(
    "Runs the tests.",
    command("cargo-nextest", &["test"], &[("PATH", "C:/bin")]),
  );
  let arguments = Tool::command(
    "Runs the tests.",
    command("cargo", &["test", "--all"], &[("PATH", "C:/bin")]),
  );
  let environment = Tool::command("Runs the tests.", command("cargo", &["test"], &[("PATH", "D:/bin")]));
  assert_eq!(
    widens(Some(&previous), &program),
    [VersionWidening::Command(CommandWidening::Program)]
  );
  assert_eq!(
    widens(Some(&previous), &arguments),
    [VersionWidening::Command(CommandWidening::Arguments)]
  );
  assert_eq!(
    widens(Some(&previous), &environment),
    [VersionWidening::Command(CommandWidening::Variable("PATH".into()))]
  );
  let removed = Tool::command("Runs the tests.", command("cargo", &["test"], &[]));
  assert_eq!(
    widens(Some(&previous), &removed),
    [VersionWidening::Command(CommandWidening::Variable("PATH".into()))],
    "removing a variable widens too"
  );
}

#[test]
fn sharing_widens_nothing_for_a_harmless_or_narrower_command_change() {
  let action = command("cargo", &["test"], &[]);
  let previous = Tool::command(
    "Runs the tests.",
    action
      .clone()
      .with_exit_codes(ExitCodes::new(BTreeSet::from([0, 1])).expect("codes")),
  );
  let narrower = Tool::command("Runs every test.", action);
  assert!(
    widens(Some(&previous), &narrower).is_empty(),
    "fewer exit codes and another description"
  );
  let logging = Tool::command("Runs the tests.", command("cargo", &["test"], &[("RUST_LOG", "debug")]));
  assert!(widens(Some(&narrower), &logging).is_empty(), "a harmless variable");
}

#[test]
fn sharing_widens_an_http_tool_whose_address_method_or_headers_changed() {
  let secret = SecretId::generate();
  let access = policy(ApprovalMode::EveryCall);
  let request = |method, url, header| ToolAction::Http(http(method, url, HttpAuth::Bearer { secret }, header));
  let previous = tool(
    request(HttpMethod::Get, "https://deploy.example.test/v1", Some("X-Token")),
    &access,
  );
  assert!(widens(Some(&previous), &previous).is_empty());
  let address = tool(
    request(HttpMethod::Get, "https://evil.example.test/v1", Some("X-Token")),
    &access,
  );
  assert_eq!(widens(Some(&previous), &address), [VersionWidening::Address]);
  let method = tool(
    request(HttpMethod::Delete, "https://deploy.example.test/v1", Some("X-Token")),
    &access,
  );
  assert_eq!(widens(Some(&previous), &method), [VersionWidening::Method]);
  let headers = tool(
    request(HttpMethod::Get, "https://deploy.example.test/v1", Some("X-Other")),
    &access,
  );
  assert_eq!(widens(Some(&previous), &headers), [VersionWidening::Headers]);
  let header_case = tool(
    request(HttpMethod::Get, "https://deploy.example.test/v1", Some("x-token")),
    &access,
  );
  assert!(
    widens(Some(&previous), &header_case).is_empty(),
    "header names ignore case"
  );
}

#[test]
fn sharing_widens_a_database_tool_whose_database_or_query_changed() {
  let (secret, access) = (SecretId::generate(), policy(ApprovalMode::EveryCall));
  let query = |secret, text: &str| ToolAction::DatabaseQuery(DatabaseQuery::new(secret, text.into()).expect("a query"));
  let previous = tool(query(secret, "SELECT 1"), &access);
  assert_eq!(
    widens(Some(&previous), &tool(query(secret, "DELETE FROM runs"), &access)),
    [VersionWidening::Query]
  );
  assert_eq!(
    widens(Some(&previous), &tool(query(SecretId::generate(), "SELECT 1"), &access)),
    [VersionWidening::Database, VersionWidening::Secret]
  );
}

#[test]
fn sharing_widens_a_new_secret_but_not_one_the_previous_version_used() {
  let access = policy(ApprovalMode::EveryCall);
  let url = "https://deploy.example.test/v1";
  let request = |auth| ToolAction::Http(http(HttpMethod::Post, url, auth, None));
  let without = tool(request(HttpAuth::None), &access);
  let secret = SecretId::generate();
  let with = tool(request(HttpAuth::Bearer { secret }), &access);
  assert_eq!(widens(Some(&without), &with), [VersionWidening::Secret]);
  assert!(widens(Some(&with), &with).is_empty(), "the same secret");
  assert!(widens(Some(&with), &without).is_empty(), "dropping the secret narrows");
  let another = tool(
    request(HttpAuth::Bearer {
      secret: SecretId::generate(),
    }),
    &access,
  );
  assert_eq!(widens(Some(&with), &another), [VersionWidening::Secret]);
}

#[test]
fn sharing_widens_a_tool_whose_access_policy_changed() {
  let action = ToolAction::Http(http(
    HttpMethod::Post,
    "https://deploy.example.test/v1",
    HttpAuth::None,
    None,
  ));
  let previous = tool(action.clone(), &policy(ApprovalMode::EveryCall));
  let unasked = tool(action, &policy(ApprovalMode::Never));
  assert_eq!(widens(Some(&previous), &unasked), [VersionWidening::Access]);
  let open = Tool::command("Runs the tests.", command("cargo", &["test"], &[]));
  let restricted = Tool::new(
    "Runs the tests.",
    open.action().clone(),
    Some(policy(ApprovalMode::EveryCall)),
  )
  .expect("a command tool with a policy");
  assert_eq!(widens(Some(&open), &restricted), [VersionWidening::Access]);
}

#[test]
fn sharing_widens_a_first_tool_by_what_it_grants() {
  let access = policy(ApprovalMode::EveryCall);
  let secret = SecretId::generate();
  let first = Tool::command(
    "Runs the tests.",
    command("cargo", &["test"], &[("PATH", "C:/bin"), ("RUST_LOG", "info")]),
  );
  assert_eq!(
    widens(None, &first),
    [
      VersionWidening::Command(CommandWidening::Program),
      VersionWidening::Command(CommandWidening::Variable("PATH".into())),
    ]
  );
  let request = tool(
    ToolAction::Http(http(
      HttpMethod::Post,
      "https://deploy.example.test/v1",
      HttpAuth::Bearer { secret },
      None,
    )),
    &access,
  );
  assert_eq!(
    widens(None, &request),
    [VersionWidening::Address, VersionWidening::Secret]
  );
  let database = tool(
    ToolAction::DatabaseQuery(DatabaseQuery::new(secret, "SELECT 1".into()).expect("a query")),
    &access,
  );
  assert_eq!(
    widens(None, &database),
    [VersionWidening::Database, VersionWidening::Secret]
  );
  assert_eq!(
    widens(Some(&first), &request),
    [
      VersionWidening::Address,
      VersionWidening::Access,
      VersionWidening::Secret
    ],
    "another kind of action grants what it does"
  );
}
