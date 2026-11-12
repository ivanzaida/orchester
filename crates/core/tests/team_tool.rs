//! The team's tool (`docs/design/team-configuration.md`, "Who owns what"): it names the secret it sends or connects
//! with by name, its JSON refuses an id where a name belongs and any field the local tool does not have, and a local
//! tool becomes one only by naming its secret.

use std::collections::BTreeMap;

use orchester_core::{
  id::SecretId,
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, ApprovalMode, CommandAction, CommandArg, CommandProgram, DatabaseQuery, DeniedBehavior,
    FieldName, HttpAuth, HttpMethod, HttpRequest, RequestTarget, TeamTool, TeamToolAction, TeamToolError, Tool,
    ToolAction, ToolKind,
  },
  secret::SecretName,
  sharing::{VersionWidening, widens},
};
use serde_json::json;

fn policy() -> AccessPolicy {
  AccessPolicy::new(
    BTreeMap::new(),
    Name::new("Production").expect("name"),
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("policy")
}

fn name(text: &str) -> SecretName {
  SecretName::new(text).expect("a secret name")
}

fn http_tool(secret: SecretId, url: &str) -> Tool {
  let request = HttpRequest::new(
    HttpMethod::Get,
    url.into(),
    "sentry.example.test".into(),
    HttpAuth::Bearer { secret },
    BTreeMap::from([(
      FieldName::new("query").expect("field"),
      RequestTarget::Query("query".into()),
    )]),
    BTreeMap::new(),
  )
  .expect("request");
  Tool::new("Issues", ToolAction::Http(request), Some(policy())).expect("tool")
}

fn database_tool(secret: SecretId) -> Tool {
  let query = DatabaseQuery::new(secret, "select 1".into()).expect("query");
  Tool::new("", ToolAction::DatabaseQuery(query), Some(policy())).expect("tool")
}

fn command_tool() -> Tool {
  Tool::command(
    "Checks.",
    CommandAction::new(
      CommandProgram::new("cargo").expect("program"),
      vec![CommandArg::new("check").expect("argument")],
    ),
  )
}

const URL: &str = "https://sentry.example.test/api/issues/";

#[test]
fn team_tool_from_local_names_the_publishers_secret_by_its_name() {
  let secret = SecretId::generate();
  let team = TeamTool::from_local(&http_tool(secret, URL), |id| {
    (id == secret).then(|| name("SENTRY_TOKEN"))
  })
  .expect("a team tool");
  assert_eq!(team.secret(), Some(&name("SENTRY_TOKEN")));
  assert_eq!(team.kind(), ToolKind::Http);
  let TeamToolAction::Http(request) = team.action() else {
    panic!("an HTTP tool: {:?}", team.action())
  };
  assert_eq!((request.method(), request.url()), (HttpMethod::Get, URL));
  assert_eq!(request.inputs().len(), 1);

  let database = TeamTool::from_local(&database_tool(secret), |_| Some(name("ORDERS_DB"))).expect("a team tool");
  assert_eq!(database.secret(), Some(&name("ORDERS_DB")));
}

#[test]
fn team_tool_from_local_refuses_a_secret_this_project_does_not_have() {
  assert_eq!(
    TeamTool::from_local(&http_tool(SecretId::generate(), URL), |_| None),
    Err(TeamToolError::SecretNotHere)
  );
}

#[test]
fn team_tool_wire_puts_a_name_where_the_local_tool_holds_an_id() {
  let secret = SecretId::generate();
  let local = http_tool(secret, URL);
  let team = TeamTool::from_local(&local, |_| Some(name("SENTRY_TOKEN"))).expect("a team tool");

  let mut expected = serde_json::to_value(&local).expect("json");
  expected["action"]["http"]["auth"]["bearer"]["secret"] = json!("SENTRY_TOKEN");
  let wire = serde_json::to_value(&team).expect("json");
  assert_eq!(wire, expected);
  assert_eq!(serde_json::from_value::<TeamTool>(wire).expect("read back"), team);
}

#[test]
fn team_tool_wire_refuses_a_local_id_and_any_other_field() {
  let secret = SecretId::generate();
  let local = serde_json::to_value(http_tool(secret, URL)).expect("json");
  assert!(
    serde_json::from_value::<TeamTool>(local).is_err(),
    "an id is no secret name"
  );
  let database = serde_json::to_value(database_tool(secret)).expect("json");
  assert!(serde_json::from_value::<TeamTool>(database).is_err());

  let team = TeamTool::from_local(&http_tool(secret, URL), |_| Some(name("SENTRY_TOKEN"))).expect("a team tool");
  let wire = serde_json::to_value(&team).expect("json");
  for (pointer, field) in [
    ("", "secret_value"),
    ("/action/http", "path"),
    ("/action/http/auth/bearer", "id"),
  ] {
    let mut extra = wire.clone();
    extra
      .pointer_mut(pointer)
      .and_then(serde_json::Value::as_object_mut)
      .expect("an object")
      .insert(field.to_owned(), json!("x"));
    assert!(
      serde_json::from_value::<TeamTool>(extra).is_err(),
      "`{field}` at `{pointer}` is refused"
    );
  }
}

#[test]
fn team_tool_wire_keeps_the_local_tools_checks() {
  let team =
    TeamTool::from_local(&http_tool(SecretId::generate(), URL), |_| Some(name("SENTRY_TOKEN"))).expect("a team tool");
  let mut plain = serde_json::to_value(&team).expect("json");
  plain["action"]["http"]["url"] = json!("http://sentry.example.test/api/issues/");
  assert!(
    serde_json::from_value::<TeamTool>(plain).is_err(),
    "a bearer token needs https"
  );
  let mut open = serde_json::to_value(&team).expect("json");
  open.as_object_mut().expect("an object").remove("access");
  assert!(
    serde_json::from_value::<TeamTool>(open).is_err(),
    "an HTTP tool needs an access policy"
  );
}

#[test]
fn team_tool_a_command_reads_the_same_in_both_forms() {
  let local = command_tool();
  let wire = serde_json::to_value(&local).expect("json");
  let team: TeamTool = serde_json::from_value(wire.clone()).expect("a command is a team tool as it is");
  assert_eq!(team.secret(), None);
  assert_eq!(serde_json::to_value(&team).expect("json"), wire);
  assert_eq!(team.command_tool(), Some(local));
}

#[test]
fn team_tool_widens_when_it_needs_another_secret_and_not_when_it_keeps_one() {
  let secret = SecretId::generate();
  let named = |text: &str| TeamTool::from_local(&http_tool(secret, URL), |_| Some(name(text))).expect("team");
  let before = named("SENTRY_TOKEN");
  assert!(widens(Some(&before), &before).is_empty());
  assert_eq!(
    widens(Some(&before), &named("OTHER_TOKEN")),
    vec![VersionWidening::Secret]
  );
  let moved = TeamTool::from_local(&http_tool(secret, "https://sentry.example.test/api/other/"), |_| {
    Some(name("SENTRY_TOKEN"))
  })
  .expect("team");
  assert_eq!(widens(Some(&before), &moved), vec![VersionWidening::Address]);
  assert_eq!(
    widens(None, &before),
    vec![VersionWidening::Address, VersionWidening::Secret]
  );
}
