//! The app's own MCP tools, served beside lurq's `lurq_*`: `app_state` reads where the window is,
//! `navigate` takes the active tab somewhere, `set_theme` switches the colors, `sign_in_state` says whether the
//! app is signed in to Orchester Cloud (the one tool that answers while it is not), and in debug builds
//! `hold_reads` holds the project feeds' reads for QA. They run on the event-loop thread, with the tree
//! and the app, like every other change to the window.

mod app_state;
mod error;
mod hold_reads;
mod navigate;
mod set_theme;
mod sign_in_state;

use lurq::mcp::McpTool;
use orchester_workspace::{links::with_usage_query, sharing::cloud::Cloud};
use serde_json::{Map, Value, json};

pub use self::{
  app_state::app_state,
  error::ToolError,
  hold_reads::hold_reads,
  navigate::navigate,
  set_theme::set_theme,
  sign_in_state::{SIGN_IN_STATE, sign_in_state},
};
use super::{McpSettings, WindowLink};
use crate::{
  appearance::Appearance,
  project::hold::ReadHold,
  routes::{OrchestratorChat, ProjectPage, Route, Screen},
};

/// Every app tool, for lurq's MCP configuration; for a started session `sign_in_state` on its `cloud`, and in debug
/// builds `hold_reads`, whose reads `hold` holds.
pub fn all(
  link: &WindowLink,
  appearance: &Appearance,
  settings: McpSettings,
  (hold, cloud): (Option<ReadHold>, Option<Cloud>),
) -> Vec<McpTool> {
  let mut tools = vec![
    app_state::tool(link.clone(), appearance.clone(), settings),
    navigate::tool(link.clone()),
    set_theme::tool(appearance.clone()),
  ];
  tools.extend(cloud.map(sign_in_state::tool));
  if cfg!(debug_assertions)
    && let Some(hold) = hold
  {
    tools.push(hold_reads::tool(hold));
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
    Route::AllProjects
    | Route::AddProject(_)
    | Route::Settings
    | Route::Accounts(_)
    | Route::Orchester(_)
    | Route::Invite(_) => {}
  }
  json!({ "name": name, "path": route.path(), "params": params })
}

fn page_params(page: &ProjectPage, params: &mut Map<String, Value>) {
  let mut insert = |name: &str, value: &str| {
    params.insert(name.to_owned(), json!(value));
  };
  match page {
    ProjectPage::Overview(tab) => insert("tab", tab.name()),
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
    ProjectPage::PullRequests(tab) => insert("tab", tab.name()),
    ProjectPage::PullRequest(number, _) => insert("number", &number.to_string()),
    ProjectPage::Usage(query) => {
      let query = with_usage_query(String::new(), query);
      if let Some(query) = query.strip_prefix('?') {
        insert("query", query);
      }
    }
    ProjectPage::Connections(view) => {
      insert("tab", view.tab().name());
      if let Some(connection) = view.selected() {
        insert("selected", &connection.to_string());
      }
    }
    ProjectPage::NewConnection(kind) => insert("kind", kind.name()),
    ProjectPage::EditConnection(connection) => insert("connection", &connection.to_string()),
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
      if let Some(file) = view.file() {
        insert("file", file.path());
        insert("from", file.source().name());
      }
    }
    ProjectPage::Files(view) => {
      if let Some(file) = view.file() {
        insert("file", file);
      }
      insert("at", &view.at().to_string());
      if let Some(line) = view.line() {
        insert("line", &line.to_string());
      }
    }
    _ => {}
  }
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
