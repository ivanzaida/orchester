//! A local project's HTTP request and database query tools (`Agents · v2` L20 `Kb6Vk`, L21 `ldRQO`, L22 `Xp89k`, L23
//! `cZGTp`, L24 `OSniP`): *New tool* makes either as v1 with the project secret it picks and its access; what the
//! request refuses is said at each field at once, with Create off; a tool's panel edits it in place, and *Save*
//! publishes its next version once a person allows what it widens.
//! `cargo test -p orchester-app --test agents local_hub_tool_http`.

use std::collections::BTreeMap;

use orchester_app::routes::{HubFilter, HubView};
use orchester_core::{
  id::{ResourceId, SecretId},
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, ApprovalMode, DeniedBehavior, HttpAuth, HttpMethod, HttpRequest, ResourceKind, Tool,
    ToolAction, VersionNumber,
  },
  store::ResourceStore,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  connections::mcp::Allowed,
  tools::editor::{self, Edit},
};

use super::{
  harness::Agents,
  local_hub::open,
  local_hub_create::{choose, held, project_secret, show_form},
};

const CREATE: &str = "local-hub-new-create";

/// The tool the project holds, its latest version.
fn only_tool(agents: &Agents) -> (VersionNumber, Tool) {
  let db = ProjectDb::open(&agents.root).expect("database");
  let resource = db
    .resources(ResourceKind::Tool)
    .expect("resources")
    .into_iter()
    .next()
    .expect("a tool");
  let version = db
    .latest_version::<Tool>(resource.id())
    .expect("read")
    .expect("a version");
  (version.number(), version.body().clone())
}

/// The form on a new HTTP request named `github-issue` that sends `GITHUB_TOKEN`, picked, as its bearer token.
fn http_form(agents: &mut Agents) {
  project_secret(agents, "GITHUB_TOKEN");
  show_form(agents, HubFilter::Tools);
  agents.set("team-new-tool-name-input", "github-issue");
  agents.click("team-new-tool-kind-value-http");
  agents.until("the request's fields", |s| s.exists("team-new-tool-url-input"));
  agents.click("team-new-tool-method-value-post");
  agents.set(
    "team-new-tool-url-input",
    "https://api.github.com/repos/acme/kontur/issues",
  );
  agents.set("team-new-tool-host-input", "api.github.com");
  agents.click("team-new-tool-auth-value-bearer");
  choose(agents, "team-new-tool-secret-input", "GITHUB_TOKEN");
}

#[test]
fn local_hub_tool_http_a_request_is_v1_with_the_secret_it_picked() {
  let mut agents = Agents::new(true);
  http_form(&mut agents);
  agents.set("team-new-tool-inputs-input", "title → body title\ndetails → body body");
  agents.set("team-new-tool-outputs-input", "number ← body number");
  agents.click(CREATE);
  agents.until("the tool is made", |s| held(s, ResourceKind::Tool) == 1);
  let (number, tool) = only_tool(&agents);
  assert_eq!(number, VersionNumber::FIRST);
  let ToolAction::Http(request) = tool.action() else {
    panic!("an HTTP tool: {tool:?}");
  };
  assert_eq!(request.method(), HttpMethod::Post);
  assert!(matches!(request.auth(), HttpAuth::Bearer { .. }));
  assert_eq!(request.inputs().len(), 2);
  let access = tool.access().expect("an access policy");
  assert!(access.callers().is_empty(), "a local tool has no pipelines to choose");
  assert_eq!(access.environment().as_str(), "Development");
}

#[test]
fn local_hub_tool_http_a_query_is_v1_with_its_credentials_and_access() {
  let mut agents = Agents::new(true);
  project_secret(&agents, "ORDERS_DB");
  show_form(&mut agents, HubFilter::Tools);
  agents.set("team-new-tool-name-input", "orders-report");
  agents.click("team-new-tool-kind-value-database");
  agents.until("the query's fields", |s| s.exists("team-new-tool-query-input"));
  choose(&mut agents, "team-new-tool-secret-input", "ORDERS_DB");
  agents.set("team-new-tool-query-input", "select id, status from orders");
  agents.click("team-new-tool-approval-value-never");
  agents.click("team-new-tool-denied-value-fail");
  agents.click(CREATE);
  agents.until("the tool is made", |s| held(s, ResourceKind::Tool) == 1);
  let (_, tool) = only_tool(&agents);
  assert!(
    matches!(tool.action(), ToolAction::DatabaseQuery(query) if query.query() == "select id, status from orders")
  );
  let access = tool.access().expect("an access policy");
  assert_eq!(
    (access.approval(), access.denied()),
    (ApprovalMode::Never, DeniedBehavior::FailStep)
  );
}

#[test]
fn local_hub_tool_http_what_the_request_refuses_is_said_at_each_field_and_create_stays_off() {
  let mut agents = Agents::new(true);
  http_form(&mut agents);
  agents.set(
    "team-new-tool-url-input",
    "http://api.github.com/repos/acme/kontur/issues",
  );
  agents.set("team-new-tool-host-input", "github.com");
  agents.set(
    "team-new-tool-inputs-input",
    "title → body title\ntoken → header Authorization",
  );
  agents.set("team-new-tool-outputs-input", "Number ← body number\nlink ← body");
  agents.until("the fields to say why", |s| {
    s.text("team-new-tool-url-sub") == "A bearer token needs an https URL"
  });
  assert_eq!(
    agents.text("team-new-tool-host-sub"),
    "The URL's host is api.github.com, not github.com"
  );
  assert_eq!(
    agents.text("team-new-tool-inputs-sub"),
    "Line 2: the Authorization header is the token's · pick another header"
  );
  assert_eq!(
    agents.text("team-new-tool-outputs-sub"),
    "Line 1: Number is not a name · Line 2: body needs a path"
  );
  assert_eq!(
    agents.text("team-hub-actions-hint"),
    "Fix the marked fields to create it"
  );
  agents.click(CREATE);
  assert_eq!(held(&agents, ResourceKind::Tool), 0, "Create is off");
  assert_eq!(
    agents.value("team-new-tool-host-input"),
    "github.com",
    "nothing typed is cleared"
  );
}

/// An HTTP request made and published as v1, which sends `secret` as its bearer token.
fn request(agents: &Agents, secret: SecretId) -> ResourceId {
  let request = HttpRequest::new(
    HttpMethod::Get,
    "https://sentry.io/api/0/issues/".into(),
    "sentry.io".into(),
    HttpAuth::Bearer { secret },
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .expect("request");
  let access = AccessPolicy::new(
    BTreeMap::new(),
    Name::new("Development").expect("name"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("access");
  let tool = Tool::new("Issues", ToolAction::Http(request), Some(access)).expect("tool");
  editor::apply_in(
    &agents.root,
    Edit::Save {
      name: Name::new("sentry-issues").expect("name"),
      tool: Box::new(tool),
      before: None,
    },
    &Allowed::Anything,
  )
  .expect("created")
}

#[test]
fn local_hub_tool_http_a_request_is_edited_in_its_panel_and_saved_once_allowed() {
  let mut agents = Agents::new(true);
  let secret = project_secret(&agents, "SENTRY_TOKEN").id();
  let id = request(&agents, secret);
  open(&mut agents, HubView::new(HubFilter::Tools, Some(id)));
  agents.until("the tool's form", |s| s.exists("local-hub-tool-url-input"));
  assert_eq!(agents.value("local-hub-tool-host-input"), "sentry.io");

  agents.set("local-hub-tool-url-input", "https://sentry.io/api/0/events/");
  agents.click("local-hub-tool-save");
  agents.until("the question", |s| s.exists("local-hub-tool-confirm-confirm"));
  agents.press_until("local-hub-tool-confirm-confirm", "published as v2", |s| {
    only_tool(s).0.get() == 2
  });
  let ToolAction::Http(saved) = only_tool(&agents).1.action().clone() else {
    panic!("an HTTP tool");
  };
  assert_eq!(saved.url(), "https://sentry.io/api/0/events/");
  assert_eq!(saved.auth(), HttpAuth::Bearer { secret }, "the secret it picked stays");
}
