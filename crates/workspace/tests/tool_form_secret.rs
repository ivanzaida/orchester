//! An HTTP or database tool's fields (A32–A35, L20–L24): a tool reads back into the same fields, the inputs and outputs
//! are lines in the app's arrows, and every field the request refuses says why at once (L22).

use std::collections::BTreeMap;

use orchester_core::{
  id::SecretId,
  resource::{
    AccessWindow, ApprovalMode, DeniedBehavior, FieldName, HttpMethod, RequestTarget, ResponseSource, TeamToolAction,
    ToolAction,
  },
  secret::Environment,
};
use orchester_workspace::tools::form::{
  AccessText, ActionText, HttpText, LineError, SecretToolText, parse_inputs, parse_outputs,
};

fn http() -> HttpText {
  HttpText {
    method: HttpMethod::Post,
    url: "https://api.github.com/repos/acme/kontur/issues".into(),
    allowed_host: "api.github.com".into(),
    bearer: true,
    inputs: "title → body title\n\ndetails -> body body".into(),
    outputs: "number ← body number\nlink <- body html_url\nstatus ← status".into(),
  }
}

fn text(action: ActionText) -> SecretToolText {
  SecretToolText {
    description: "Opens an issue".into(),
    action,
    access: AccessText {
      environment: Some(Environment::Production),
      by_minutes: true,
      minutes: "15".into(),
      ..AccessText::default()
    },
  }
}

fn keys(errors: &[LineError]) -> Vec<&str> {
  errors.iter().map(|error| error.reason.key).collect()
}

#[test]
fn tool_form_secret_http_becomes_a_tool_and_reads_back() {
  let secret = SecretId::generate();
  let typed = text(ActionText::Http(http()));
  let tool = typed.tool(Some(secret)).expect("valid fields");
  let ToolAction::Http(request) = tool.action() else {
    panic!("an HTTP tool");
  };
  assert_eq!(request.inputs().len(), 2);
  assert_eq!(
    request.outputs().get(&FieldName::new("link").expect("name")),
    Some(&ResponseSource::Body("html_url".into()))
  );
  assert_eq!(tool.secret(), Some(secret));
  let access = tool.access().expect("an access policy");
  assert_eq!(access.environment().as_str(), "Production");
  assert_eq!(access.window(), AccessWindow::Minutes(15.try_into().expect("minutes")));
  assert_eq!(access.approval(), ApprovalMode::OncePerRun);
  assert_eq!(access.denied(), DeniedBehavior::AwaitChangedPlan);
  let (read, picked) = SecretToolText::of(&tool).expect("an HTTP tool");
  assert_eq!(picked, Some(secret));
  assert_eq!(read.tool(picked), Ok(tool));
  let ActionText::Http(read) = read.action else {
    panic!("an HTTP tool");
  };
  assert_eq!(read.inputs, "details → body body\ntitle → body title");
}

#[test]
fn tool_form_secret_team_tools_name_their_secret() {
  let tool = text(ActionText::Http(http())).team_tool("GITHUB_TOKEN").expect("valid");
  assert_eq!(tool.secret().map(|name| name.as_str()), Some("GITHUB_TOKEN"));
  let (read, named) = SecretToolText::of_team(&tool).expect("an HTTP tool");
  assert_eq!(read.team_tool(named.expect("named").as_str()), Ok(tool));
  let query = text(ActionText::Database("select 1".into()))
    .team_tool("ORDERS_DB")
    .expect("valid");
  assert!(matches!(query.action(), TeamToolAction::DatabaseQuery(q) if q.query() == "select 1"));
  let errors = text(ActionText::Database(" ".into()))
    .team_tool("orders")
    .expect_err("blank query, lowercase name");
  assert_eq!(errors.query.map(|m| m.key), Some("tools.errors.queryRequired"));
  assert_eq!(errors.secret.map(|m| m.key), Some("tools.errors.secretName"));
}

#[test]
fn tool_form_secret_says_every_wrong_field_at_once() {
  let wrong = HttpText {
    url: "http://api.github.com/repos/acme/kontur/issues".into(),
    allowed_host: "github.com".into(),
    inputs: "title → body title\ntoken → header Authorization".into(),
    outputs: "Number ← body number\nlink ← body".into(),
    ..http()
  };
  let errors = text(ActionText::Http(wrong)).tool(None).expect_err("wrong fields");
  assert_eq!(errors.http.url.map(|m| m.key), Some("tools.errors.insecureBearer"));
  let host = errors.http.allowed_host.expect("host error");
  assert_eq!(host.key, "tools.errors.hostNotAllowed");
  assert_eq!(
    host.args,
    vec![("host", "api.github.com".into()), ("allowed", "github.com".into())]
  );
  assert_eq!(keys(&errors.http.inputs), ["tools.errors.line.reserved"]);
  assert_eq!(
    keys(&errors.http.outputs),
    ["tools.errors.line.name", "tools.errors.line.needsPath"]
  );
  assert_eq!(
    errors.http.outputs.iter().map(|error| error.line).collect::<Vec<_>>(),
    [1, 2]
  );
  assert_eq!(errors.secret.map(|m| m.key), Some("tools.errors.secretRequired"));
}

#[test]
fn tool_form_secret_lines_and_access_refuse_what_they_cannot_hold() {
  let refused = parse_inputs("a → cookie x\nb query\nb → query q\nb → query r").expect_err("wrong lines");
  assert_eq!(
    keys(&refused),
    [
      "tools.errors.line.inputPlace",
      "tools.errors.line.inputShape",
      "tools.errors.line.twice"
    ]
  );
  assert_eq!(
    parse_inputs("q → query per_page"),
    Ok(BTreeMap::from([(
      FieldName::new("q").expect("name"),
      RequestTarget::Query("per_page".into())
    )]))
  );
  let outputs = parse_outputs("s ← status 200\nh ← header Bad Name").expect_err("wrong lines");
  assert_eq!(
    keys(&outputs),
    ["tools.errors.line.statusKey", "tools.errors.line.invalidName"]
  );
  let url = HttpText {
    url: "ftp://example.com".into(),
    allowed_host: String::new(),
    ..http()
  };
  let errors = text(ActionText::Http(url))
    .tool(Some(SecretId::generate()))
    .expect_err("wrong");
  assert_eq!(errors.http.url.map(|m| m.key), Some("tools.errors.invalidUrl"));
  assert_eq!(
    errors.http.allowed_host.map(|m| m.key),
    Some("tools.errors.hostRequired")
  );
  let access = AccessText {
    environment: None,
    by_minutes: true,
    minutes: "0".into(),
    ..AccessText::default()
  };
  let errors = access.policy().expect_err("no environment, no minutes");
  assert_eq!(
    errors.environment.map(|m| m.key),
    Some("tools.errors.environmentRequired")
  );
  assert_eq!(errors.minutes.map(|m| m.key), Some("tools.errors.minutes"));
}
