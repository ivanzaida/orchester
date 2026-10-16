use std::num::NonZeroU64;

use lurq::mcp::{McpTool, Scope};
use orchester_core::{
  id::{ConnectionId, ProjectId},
  project::{TaskKey, TaskKeyError},
};
use orchester_workspace::{
  files::{LineRangeError, Version, VersionParseError},
  pull_requests::list::PullRequestTab,
};
use serde_json::{Map, Value, json};

use super::{ToolError, WindowLink, object, reply, route_json, string};
use crate::{
  navigation::Navigation,
  routes::{
    ChangeSource, ChangedFile, ConnectionsTab, ConnectionsView, FilesView, NewConnection, OrchestratorChat,
    OverviewTab, ProjectPage, Route, Screen, TaskList, TaskListTab, TaskTab, TaskView,
  },
};

const DESCRIPTION: &str = "Take the active tab of the Orchester window to a route, as a new entry of its history \
  (the mouse's back button, or Alt+Left, Cmd+[ on macOS, returns). `route` is the screen's name: all_projects and \
  settings take no other argument; every other screen takes `project`, the project's id (a UUID, as app_state \
  reports it). overview also takes `tab`; tasks `tab`, `q` (the search) and `highlight` (a task key, as after \
  Create); edit_task `key`; task `key`, `tab` and, on its changes tab, `file` with `from`; connections `tab` and \
  `selected` (the id of the connection its inspector shows); new_connection `kind`; edit_connection `connection` \
  (an MCP server's id); tools takes tool (a tool id, or new), and its editor tab; orchestrator takes `chat` (a \n  conversation's id, or new; left out, the latest); files takes `file` (a path in the project; the tree alone when \
  left out), `at` (checkout, the default, base, run:<run id> or rev:<commit>) and `line` (12 or 12-20); pull_requests \
  takes `tab`, or `number` for one pull request; usage takes `query`, the page's query as a link carries it, and \
  without `project` shows every project's usage. A tab or kind left out is the one the screen opens on. Answers the \
  route the tab shows, whose params navigate takes back. Errors are JSON with \
  a stable `code`: unknown_route (with the known names), missing_param, unexpected_param, invalid_param, \
  unknown_tab (with the screen's tabs), not_a_string, not_an_object, not_ready (the window is starting).";

pub fn tool(link: WindowLink) -> McpTool {
  McpTool::new("navigate")
    .description(DESCRIPTION)
    .scope(Scope::Interact)
    .input_schema(schema())
    .handler(move |_ctx, args| reply(link.navigation().and_then(|navigation| navigate(&navigation, &args))))
}

fn schema() -> Value {
  let names: Vec<&str> = Screen::ALL.into_iter().map(Screen::name).collect();
  let tabs = format!(
    "The screen's tab, the one it opens on when left out: overview {}; tasks {}; task {}; connections {}; \
     pull_requests {}.",
    OverviewTab::ALL.map(OverviewTab::name).join(", "),
    TaskListTab::ALL.map(TaskListTab::name).join(", "),
    TaskTab::ALL.map(TaskTab::name).join(", "),
    ConnectionsTab::ALL.map(ConnectionsTab::name).join(", "),
    PullRequestTab::ALL.map(PullRequestTab::name).join(", "),
  );
  let kinds: Vec<&str> = NewConnection::ALL.into_iter().map(NewConnection::name).collect();
  let sources: Vec<&str> = ChangeSource::ALL.into_iter().map(ChangeSource::name).collect();
  json!({
    "type": "object",
    "properties": {
      "route": { "type": "string", "enum": names },
      "project": { "type": "string", "description": "The project's id (a UUID), for every screen of a project." },
      "key": { "type": "string", "description": "The task's key, e.g. KON-12, for task and edit_task." },
      "tab": { "type": "string", "description": tabs },
      "q": { "type": "string", "description": "tasks: the text searched for." },
      "highlight": { "type": "string", "description": "tasks: the key of the task to highlight, as after Create." },
      "file": {
        "type": "string",
        "description": "task: the repository path of a changed file to show on the changes tab (the tab when left out). files: the path of the file to show (the tree alone when left out).",
      },
      "selected": { "type": "string", "description": "connections: the id of the connection its inspector shows." },
      "kind": { "type": "string", "enum": kinds, "description": "new_connection: what it adds." },
      "connection": { "type": "string", "description": "edit_connection: the connection's id: an MCP server's form, or an agent CLI connection's account." },
      "skill": { "type": "string", "description": "skills: the skill id, new, or import; omitted opens the library." },
      "tool": { "type": "string", "description": "tools: the tool’s id, or new for a command draft; omitted opens the library." },
      "chat": { "type": "string", "description": "orchestrator: a conversation's id, or new; omitted opens the latest." },
      "at": { "type": "string", "description": "files: the version, checkout (the default), base, run:<run id> or rev:<commit>." },
      "line": { "type": "string", "description": "files: the line or lines to show, 12 or 12-20." },
      "number": { "type": "string", "description": "pull_requests: the number of the pull request to open, e.g. 142." },
      "query": {
        "type": "string",
        "description": "usage: the period, grouping and filters as a link's query carries them, e.g. `from=2026-10-01&to=2026-10-15&group=task&model=gpt-5` (get_usage's `link`); omitted, the last 7 days by day.",
      },
      "from": {
        "type": "string",
        "enum": sources,
        "description": "With `file`: its change at the run's latest checkpoint (the default) or in its worktree after it.",
      },
    },
    "required": ["route"],
    "additionalProperties": false,
  })
}

/// Takes the active tab to the route `args` name and answers the route it shows.
pub fn navigate(navigation: &Navigation, args: &Value) -> Result<Value, ToolError> {
  let route = target(&object(args)?)?;
  navigation.go(route);
  Ok(json!({ "route": route_json(&navigation.route_now()) }))
}

/// The route `args` name: `route`, and the parameters its screen takes.
fn target(args: &Map<String, Value>) -> Result<Route, ToolError> {
  let name = string(args, "route")?.ok_or(ToolError::Missing { param: "route" })?;
  let screen = Screen::from_name(name).ok_or_else(|| ToolError::UnknownRoute { name: name.to_owned() })?;
  let takes = params(screen);
  if let Some(param) = args
    .keys()
    .find(|key| *key != "route" && !takes.contains(&key.as_str()))
  {
    return Err(ToolError::UnexpectedParam {
      of: screen.name(),
      param: param.clone(),
    });
  }
  let page = match screen {
    Screen::AllProjects => return Ok(Route::AllProjects),
    Screen::AddProject => return Ok(Route::AddProject(None)),
    Screen::Settings => return Ok(Route::Settings),
    Screen::Overview => ProjectPage::Overview(tab(
      args,
      OverviewTab::from_name,
      &OverviewTab::ALL.map(OverviewTab::name),
    )?),
    Screen::Orchestrator => ProjectPage::Orchestrator(orchestrator_chat(args)?),
    Screen::OrchestratorSettings => ProjectPage::OrchestratorSettings,
    Screen::Tasks => ProjectPage::Tasks(task_list(args)?),
    Screen::NewTask => ProjectPage::NewTask,
    Screen::EditTask => ProjectPage::EditTask(key(args, "key")?),
    Screen::Task => ProjectPage::Task(key(args, "key")?, task_view(args)?),
    Screen::MergeQueue => ProjectPage::MergeQueue,
    Screen::PullRequests => pull_requests_page(args)?,
    Screen::Usage => match string(args, "project")? {
      Some(_) => usage_page(args)?,
      None => return usage_everywhere(args),
    },
    Screen::Pipelines => ProjectPage::Pipelines,
    Screen::Agents => ProjectPage::Agents,
    Screen::Instructions => ProjectPage::Instructions,
    Screen::Memory => ProjectPage::Memory,
    Screen::Skills => skills_page(args)?,
    Screen::Tools => tools_page(args)?,
    Screen::Secrets => ProjectPage::Secrets,
    Screen::Connections => ProjectPage::Connections(connections_view(args)?),
    Screen::NewConnection => ProjectPage::NewConnection(new_connection(args)?),
    Screen::EditConnection => ProjectPage::EditConnection(connection(args, "connection")?),
    Screen::ProjectSettings => ProjectPage::Settings,
    Screen::Files => ProjectPage::Files(files_view(args)?),
  };
  Ok(Route::Project(project(args)?, page))
}

/// The parameters besides `route` that `screen` takes.
const fn params(screen: Screen) -> &'static [&'static str] {
  match screen {
    Screen::AllProjects | Screen::AddProject | Screen::Settings => &[],
    Screen::Overview => &["project", "tab"],
    Screen::Tasks => &["project", "tab", "q", "highlight"],
    Screen::EditTask => &["project", "key"],
    Screen::Task => &["project", "key", "tab", "file", "from"],
    Screen::Connections => &["project", "tab", "selected"],
    Screen::Tools => &["project", "tool", "tab"],
    Screen::Skills => &["project", "skill"],
    Screen::Orchestrator => &["project", "chat"],
    Screen::Files => &["project", "file", "at", "line"],
    Screen::PullRequests => &["project", "tab", "number"],
    Screen::NewConnection => &["project", "kind"],
    Screen::EditConnection => &["project", "connection"],
    Screen::Usage => &["project", "query"],
    Screen::NewTask
    | Screen::MergeQueue
    | Screen::Pipelines
    | Screen::Agents
    | Screen::Instructions
    | Screen::Memory
    | Screen::Secrets
    | Screen::OrchestratorSettings
    | Screen::ProjectSettings => &["project"],
  }
}

fn files_view(args: &Map<String, Value>) -> Result<FilesView, ToolError> {
  let at = match string(args, "at")? {
    Some(at) => at.parse().map_err(|error: VersionParseError| ToolError::InvalidParam {
      param: "at",
      reason: error.to_string(),
    })?,
    None => Version::Checkout,
  };
  let line = string(args, "line")?
    .map(|line| {
      line.parse().map_err(|error: LineRangeError| ToolError::InvalidParam {
        param: "line",
        reason: error.to_string(),
      })
    })
    .transpose()?;
  Ok(FilesView::new(string(args, "file")?, at).with_line(line))
}

fn tools_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let Some(tool) = string(args, "tool")? else {
    return Ok(ProjectPage::Tools);
  };
  if tool == "new" {
    return Ok(ProjectPage::NewTool);
  }
  let id = tool.parse().map_err(|e: uuid::Error| ToolError::InvalidParam {
    param: "tool",
    reason: e.to_string(),
  })?;
  let chosen = tab(
    args,
    crate::routes::ToolTab::from_name,
    &crate::routes::ToolTab::ALL.map(crate::routes::ToolTab::name),
  )?;
  Ok(ProjectPage::Tool(id, chosen))
}

/// The Usage page with the query `query` gives, as its path reads it.
fn usage_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let query = string(args, "query")?.unwrap_or_default();
  match Route::parse(&format!("/project/{}/usage?{query}", project(args)?)) {
    Ok(Route::Project(_, page @ ProjectPage::Usage(_))) => Ok(page),
    Ok(_) => Err(ToolError::InvalidParam {
      param: "query",
      reason: format!("`{query}` names another screen"),
    }),
    Err(error) => Err(ToolError::InvalidParam {
      param: "query",
      reason: error.to_string(),
    }),
  }
}

/// Every project's usage with the query `query` gives (All projects' *Usage* tab).
fn usage_everywhere(args: &Map<String, Value>) -> Result<Route, ToolError> {
  let query = string(args, "query")?.unwrap_or_default();
  match Route::parse(&format!("/usage?{query}")) {
    Ok(route @ Route::Usage(_)) => Ok(route),
    Ok(_) => Err(ToolError::InvalidParam {
      param: "query",
      reason: format!("`{query}` names another screen"),
    }),
    Err(error) => Err(ToolError::InvalidParam {
      param: "query",
      reason: error.to_string(),
    }),
  }
}

fn project(args: &Map<String, Value>) -> Result<ProjectId, ToolError> {
  let value = string(args, "project")?.ok_or(ToolError::Missing { param: "project" })?;
  value.parse().map_err(|error: uuid::Error| ToolError::InvalidParam {
    param: "project",
    reason: format!("`{value}` is not a project id ({error})"),
  })
}

/// The task key in `param`, which is required.
fn key(args: &Map<String, Value>, param: &'static str) -> Result<TaskKey, ToolError> {
  let value = string(args, param)?.ok_or(ToolError::Missing { param })?;
  value.parse().map_err(|error: TaskKeyError| ToolError::InvalidParam {
    param,
    reason: error.to_string(),
  })
}

/// The tab `tab` names, read by `from_name`, or the one the screen opens on when it names none; a
/// refusal lists the screen's tabs, `known`.
fn tab<T: Default>(
  args: &Map<String, Value>,
  from_name: fn(&str) -> Option<T>,
  known: &[&'static str],
) -> Result<T, ToolError> {
  let Some(given) = string(args, "tab")? else {
    return Ok(T::default());
  };
  from_name(given).ok_or_else(|| ToolError::UnknownTab {
    name: given.to_owned(),
    known: known.to_vec(),
  })
}

/// The connection id in `param`, which is required.
fn connection(args: &Map<String, Value>, param: &'static str) -> Result<ConnectionId, ToolError> {
  let value = string(args, param)?.ok_or(ToolError::Missing { param })?;
  value.parse().map_err(|error: uuid::Error| ToolError::InvalidParam {
    param,
    reason: format!("`{value}` is not a connection id ({error})"),
  })
}

/// Pull request `number`, or the page on `tab`; not both.
fn pull_requests_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let Some(number) = string(args, "number")? else {
    let known = PullRequestTab::ALL.map(PullRequestTab::name);
    return Ok(ProjectPage::PullRequests(tab(args, PullRequestTab::from_name, &known)?));
  };
  if args.contains_key("tab") {
    return Err(ToolError::InvalidParam {
      param: "number",
      reason: "a pull request is shown without the list's tab".to_owned(),
    });
  }
  number
    .trim_start_matches('#')
    .parse::<NonZeroU64>()
    .map(ProjectPage::PullRequest)
    .map_err(|_| ToolError::InvalidParam {
      param: "number",
      reason: format!("`{number}` is not a pull request number"),
    })
}

fn connections_view(args: &Map<String, Value>) -> Result<ConnectionsView, ToolError> {
  let tab = tab(
    args,
    ConnectionsTab::from_name,
    &ConnectionsTab::ALL.map(ConnectionsTab::name),
  )?;
  let view = ConnectionsView::new(tab);
  if args.contains_key("selected") {
    return Ok(view.with_selected(connection(args, "selected")?));
  }
  Ok(view)
}

/// What `kind` adds, an agent provider when left out.
fn new_connection(args: &Map<String, Value>) -> Result<NewConnection, ToolError> {
  let Some(name) = string(args, "kind")? else {
    return Ok(NewConnection::default());
  };
  NewConnection::from_name(name).ok_or_else(|| ToolError::InvalidParam {
    param: "kind",
    reason: format!(
      "`{name}` is not one of {}",
      NewConnection::ALL.map(NewConnection::name).join(", ")
    ),
  })
}

fn task_list(args: &Map<String, Value>) -> Result<TaskList, ToolError> {
  let tab = tab(args, TaskListTab::from_name, &TaskListTab::ALL.map(TaskListTab::name))?;
  let mut list = TaskList::new(tab);
  if let Some(text) = string(args, "q")? {
    list = list.with_search(text);
  }
  if args.contains_key("highlight") {
    list = list.with_highlight(key(args, "highlight")?);
  }
  Ok(list)
}

/// The task's tab, and on Changes the file `file` and `from` name; a file alone opens Changes.
fn task_view(args: &Map<String, Value>) -> Result<TaskView, ToolError> {
  let (file, from) = (string(args, "file")?, string(args, "from")?);
  let tab = tab(args, TaskTab::from_name, &TaskTab::ALL.map(TaskTab::name))?;
  let Some(path) = file else {
    if from.is_some() {
      return Err(ToolError::InvalidParam {
        param: "from",
        reason: "it goes with `file` and says where that file's change is read from".to_owned(),
      });
    }
    return Ok(TaskView::of(tab));
  };
  if args.contains_key("tab") && tab != TaskTab::Changes {
    return Err(ToolError::InvalidParam {
      param: "file",
      reason: "a file is shown on the changes tab only".to_owned(),
    });
  }
  let source = match from {
    None => ChangeSource::default(),
    Some(name) => ChangeSource::from_name(name).ok_or_else(|| ToolError::InvalidParam {
      param: "from",
      reason: format!(
        "`{name}` is not one of {}",
        ChangeSource::ALL.map(ChangeSource::name).join(", ")
      ),
    })?,
  };
  let file = ChangedFile::new(path, source).map_err(|_| ToolError::InvalidParam {
    param: "file",
    reason: "the path is empty".to_owned(),
  })?;
  Ok(TaskView::Changes(Some(file)))
}
fn orchestrator_chat(args: &Map<String, Value>) -> Result<OrchestratorChat, ToolError> {
  match string(args, "chat")? {
    None => Ok(OrchestratorChat::Latest),
    Some("new") => Ok(OrchestratorChat::New),
    Some(id) => id
      .parse()
      .map(OrchestratorChat::Chat)
      .map_err(|e: uuid::Error| ToolError::InvalidParam {
        param: "chat",
        reason: e.to_string(),
      }),
  }
}

fn skills_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  match string(args, "skill")? {
    None => Ok(ProjectPage::Skills),
    Some("new") => Ok(ProjectPage::NewSkill),
    Some("import") => Ok(ProjectPage::ImportSkills),
    Some(id) => id
      .parse()
      .map(ProjectPage::Skill)
      .map_err(|e: uuid::Error| ToolError::InvalidParam {
        param: "skill",
        reason: e.to_string(),
      }),
  }
}
