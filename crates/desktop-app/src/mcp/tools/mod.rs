//! The app's own MCP tools, served beside lurq's `lurq_*`: `app_state` reads where the window is,
//! `navigate` takes the active tab somewhere, `set_theme` switches the colors, `sign_in_state` says whether the
//! app is signed in to Orchester Cloud (the one tool that answers while it is not), `update_state` where this copy's
//! own update stands, and in debug builds `hold_reads` holds the project feeds' reads and `set_update_state` shows
//! any update state, for QA. They run on the event-loop thread, with the tree and the app, like every other change
//! to the window.

mod app_state;
mod error;
mod hold_reads;
mod navigate;
mod project_settings_route;
mod set_theme;
mod set_update_state;
mod settings_route;
mod sign_in_state;
mod update_state;

use lurq::mcp::McpTool;
use orchester_workspace::links::with_usage_query;
use serde_json::{Map, Value, json};

pub use self::{
  app_state::app_state,
  error::ToolError,
  hold_reads::hold_reads,
  navigate::{navigate, navigate_with},
  set_theme::set_theme,
  set_update_state::set_update_state,
  settings_route::Lookup,
  sign_in_state::{SIGN_IN_STATE, sign_in_state},
  update_state::update_state,
};
use super::{McpSettings, WindowLink};
use crate::{
  appearance::Appearance,
  routes::{AccountsPanel, ConnectStep, OrchesterAsk, OrchesterPanel, OrchestratorChat, ProjectPage, Route, Screen},
  services::Services,
};

/// Every app tool, for lurq's MCP configuration; for a started `session` `sign_in_state` on its cloud and `navigate`
/// finding Settings' accounts and team servers in its data, and for a `debug_build` the tools only debug builds serve,
/// for QA: `hold_reads`, whose reads its hold holds, and `set_update_state` on its updates.
pub fn all(
  link: &WindowLink,
  appearance: &Appearance,
  settings: McpSettings,
  session: Option<&Services>,
  debug_build: bool,
) -> Vec<McpTool> {
  let lookup = session.map(Lookup::of);
  let mut tools = vec![
    app_state::tool(link.clone(), appearance.clone(), settings),
    navigate::tool(link.clone(), lookup),
    set_theme::tool(appearance.clone()),
  ];
  tools.extend(session.map(|services| sign_in_state::tool(services.cloud.clone())));
  tools.extend(session.map(|services| update_state::tool(services.updates.clone())));
  if debug_build && let Some(services) = session {
    tools.push(hold_reads::tool(services.hold.clone()));
    tools.push(set_update_state::tool(services.updates.clone()));
  }
  tools
}

/// What lurq answers for a tool: its value, or its error as JSON text.
fn reply(result: Result<Value, ToolError>) -> Result<Value, String> {
  result.map_err(|error| error.to_json().to_string())
}

/// A route as the tools show it: its screen's name, its path and the parameters `navigate` takes, a tab
/// always and the others when the route has them.
pub fn route_json(route: &Route) -> Value {
  let mut params = Map::new();
  let mut name = route.screen().name();
  match route {
    Route::Project(project, page) => {
      params.insert("project".to_owned(), json!(project.to_string()));
      page_params(page, &mut params);
    }
    // Every project's usage: navigate takes it back as usage without a project.
    Route::Usage(query) => {
      name = Screen::Usage.name();
      if let Some(query) = with_usage_query(String::new(), query).strip_prefix('?') {
        params.insert("query".to_owned(), json!(query));
      }
    }
    Route::General => {
      params.insert("tab".to_owned(), json!("general"));
    }
    Route::Updates(show) => {
      params.insert("tab".to_owned(), json!(show.name()));
    }
    Route::Notifications => {
      params.insert("tab".to_owned(), json!("notifications"));
    }
    Route::RotateToken => {
      params.insert("tab".to_owned(), json!("automation"));
      params.insert("ask".to_owned(), json!("rotate-token"));
    }
    Route::Accounts(view) => {
      params.insert("tab".to_owned(), json!("accounts"));
      if let Some(account) = view.account {
        params.insert("account".to_owned(), json!(account.to_string()));
      }
      if view.panel != AccountsPanel::Account {
        params.insert("panel".to_owned(), json!(view.panel.name()));
      }
    }
    Route::Orchester(view) => orchester_params(*view, &mut params),
    Route::Invite(target) => {
      params.insert("tab".to_owned(), json!("invite"));
      params.insert("server".to_owned(), json!(target.server.to_string()));
      params.insert("project".to_owned(), json!(target.slug()));
    }
    Route::AllProjects | Route::AddProject(_) | Route::Settings => {}
  }
  json!({ "name": name, "path": route.path(), "params": params })
}

/// Orchester Cloud's place as navigate takes it: its tab (connect for *Connect a server*), the server, the step and
/// the question asked in place.
fn orchester_params(view: crate::routes::OrchesterView, params: &mut Map<String, Value>) {
  let connect = view.panel == OrchesterPanel::Connect;
  params.insert("tab".to_owned(), json!(if connect { "connect" } else { "orchester" }));
  if let Some(server) = view.server {
    params.insert("server".to_owned(), json!(server.to_string()));
  }
  if connect && view.step != ConnectStep::Address {
    params.insert("step".to_owned(), json!(view.step.name()));
  }
  match view.ask {
    Some(OrchesterAsk::SignOut) => {
      params.insert("ask".to_owned(), json!("sign-out"));
    }
    Some(OrchesterAsk::Decline(invite)) => {
      params.insert("ask".to_owned(), json!("decline"));
      params.insert("invite".to_owned(), json!(invite.to_string()));
    }
    None => {}
  }
}

fn page_params(page: &ProjectPage, params: &mut Map<String, Value>) {
  let mut insert = |name: &str, value: &str| {
    params.insert(name.to_owned(), json!(value));
  };
  match page {
    ProjectPage::Overview(tab) => insert("tab", tab.name()),
    ProjectPage::Import(tab) => insert("tab", tab.name()),
    ProjectPage::Orchestrator(OrchestratorChat::New) => insert("chat", "new"),
    ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)) => insert("chat", &chat.to_string()),
    ProjectPage::Tasks(list) => {
      insert("tab", list.tab().name());
      if let Some(text) = list.search() {
        insert("q", text);
      }
      if let Some(key) = list.highlight() {
        insert("highlight", &key.to_string());
      }
    }
    ProjectPage::EditTask(key) => insert("key", &key.to_string()),
    ProjectPage::Memory(Some(tab)) => insert("tab", tab.name()),
    ProjectPage::PullRequests(tab) => insert("tab", tab.name()),
    ProjectPage::PullRequest(number, _) => insert("number", &number.to_string()),
    ProjectPage::PullRequestFeedback(number, _) => {
      insert("number", &number.to_string());
      insert("tab", "feedback");
    }
    ProjectPage::Usage(query) => {
      let query = with_usage_query(String::new(), query);
      if let Some(query) = query.strip_prefix('?') {
        insert("query", query);
      }
    }
    ProjectPage::Delivery(period) => {
      insert("view", "delivery");
      insert("period", period.name());
    }
    ProjectPage::Connections(view) => {
      if let Some(connection) = view.selected() {
        insert("selected", &connection.to_string());
      }
    }
    ProjectPage::Settings(tab) => insert("tab", tab.name()),
    ProjectPage::ProjectRules => {
      insert("tab", crate::routes::ProjectSettingsTab::Runs.name());
      insert("view", "rules");
    }
    ProjectPage::NewConnection(kind) => insert("kind", kind.name()),
    ProjectPage::EditConnection(connection) => insert("connection", &connection.to_string()),
    ProjectPage::NewAgent(_) => insert("agent", "new"),
    ProjectPage::Agent(id, _) => insert("agent", &id.to_string()),
    ProjectPage::NewSkill => insert("skill", "new"),
    ProjectPage::Skill(id) => insert("skill", &id.to_string()),
    ProjectPage::ImportSkills => insert("skill", "import"),
    ProjectPage::NewTool => insert("tool", "new"),
    ProjectPage::Tool(tool, tab) => {
      insert("tool", &tool.to_string());
      insert("tab", tab.name());
    }
    ProjectPage::Task(key, view) | ProjectPage::KeptTask(key, view) => {
      if matches!(page, ProjectPage::KeptTask(..)) {
        insert("kept", "true");
      }
      insert("key", &key.to_string());
      insert("tab", view.tab().name());
      if let Some(sub) = view.sub_name() {
        insert("view", sub);
      }
      if let Some(file) = view.file() {
        insert("file", file.path());
        insert("from", file.source().name());
      }
    }
    page => {
      for (name, value) in module_params(page).into_iter().chain(files_params(page)) {
        insert(name, &value);
      }
    }
  }
}

/// The parameters `navigate` takes back for a Modules place: the list's `tab`; `module` new; a module's id with its
/// `tab`, or with `view` edit; and New task in module's `module`.
fn module_params(page: &ProjectPage) -> Vec<(&'static str, String)> {
  match page {
    ProjectPage::Modules(tab) => vec![("tab", tab.name().to_owned())],
    ProjectPage::NewModule => vec![("module", "new".to_owned())],
    ProjectPage::Module(module, tab) => vec![("module", module.to_string()), ("tab", tab.name().to_owned())],
    ProjectPage::EditModule(module) => vec![("module", module.to_string()), ("view", "edit".to_owned())],
    ProjectPage::NewTaskInModule(module) => vec![("module", module.to_string())],
    _ => Vec::new(),
  }
}

/// The parameters `navigate` takes back for Files: the `file` shown, its version `at` and its `line`.
fn files_params(page: &ProjectPage) -> Vec<(&'static str, String)> {
  let ProjectPage::Files(view) = page else {
    return Vec::new();
  };
  let mut params: Vec<(&'static str, String)> = view.file().map(|file| ("file", file.to_owned())).into_iter().collect();
  params.push(("at", view.at().to_string()));
  params.extend(view.line().map(|line| ("line", line.to_string())));
  params
}

/// The string argument `name`, if given.
fn string<'a>(args: &'a Map<String, Value>, name: &str) -> Result<Option<&'a str>, ToolError> {
  match args.get(name) {
    None => Ok(None),
    Some(Value::String(value)) => Ok(Some(value)),
    Some(_) => Err(ToolError::NotAString { param: name.to_owned() }),
  }
}

/// The arguments as an object; no arguments at all are an empty one.
fn object(args: &Value) -> Result<Map<String, Value>, ToolError> {
  match args {
    Value::Null => Ok(Map::new()),
    Value::Object(args) => Ok(args.clone()),
    _ => Err(ToolError::NotAnObject),
  }
}
