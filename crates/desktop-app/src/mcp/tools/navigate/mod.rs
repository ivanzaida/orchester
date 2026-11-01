mod join_team;
mod modules;

use std::num::NonZeroU64;

use lurq::mcp::{McpTool, Scope};
use modules::{agent_page, module_page, new_task_page};
use orchester_core::{
  id::{ConnectionId, ProjectId},
  project::{TaskKey, TaskKeyError},
};
use orchester_workspace::{
  delivery::DeliveryPeriod,
  files::{LineRangeError, Version, VersionParseError},
  pull_requests::list::PullRequestTab,
};
use serde_json::{Map, Value, json};

use super::{
  ToolError, WindowLink, object,
  project_settings_route::{SETTINGS_TABS, connections_view, new_connection, project_settings_page},
  reply, route_json,
  settings_route::{self, Lookup},
  string,
};
use crate::{
  navigation::Navigation,
  routes::{
    ChangeSource, ChangedFile, FilesView, ImportTab, InstructionsView, MemoryTab, ModuleListTab, ModuleTab,
    NewConnection, OrchestratorChat, OverviewTab, ProjectPage, Route, Screen, SecretsView, SkillsFilter, TaskList,
    TaskListTab, TaskTab, TaskView,
  },
};

const DESCRIPTION: &str = "Take the active tab of the Orchester window to a route, as a new entry of its history \
  (the mouse's back button, or Alt+Left, Cmd+[ on macOS, returns). `route` is the screen's name: all_projects takes no \
  other argument; join_team takes `way` (cloud, the default: the account's teams and invites; code: the code page; \
  projects with `server`: a joined team's shared projects; folder with `server` and `project`: link one to a folder); \
  settings takes `tab` (automation, general, updates or whats-new for General with its Updates in \n  view and, with whats-new, the release notes open, notifications, accounts, browser, orchester for Orchester \n  Cloud, connect for Connect a server, invite for an invite's landing) and what that tab opens on: accounts \n  `account` (an account's id or name, or claude-default / codex-default: its page) and `panel` (add for Add account \
  without `account`; sign_in, rename or remove for that account's step 2, name field or Remove question); orchester \
  `server` (a team server's id, name or host: its page) and `ask` (sign-out: Sign outÃ¢â‚¬Â¦? under the account card, or \
  on that server's page; decline with `invite`, an invite's id: Decline the invite?); connect `server` and `step` \
  (address, compare, join or username: past address it asks what answers at that server's address, and a server \
  with its own certificate stays at compare, which only a person answers); invite `server` and `project` (the \
  project's slug there); automation `ask` rotate-token (Rotate the token?). Opening a page or a question is \
  navigation; answering it stays a person's. Every other screen takes `project`, the project's id (a \
  UUID, as app_state reports it). overview, memory and import_resources also take `tab`; tasks `tab`, `q` (the \
  search) and `highlight` (a task key, as after Create), and shows the list or the board as last chosen for the \
  project (press tasks-layout-list or tasks-layout-board to switch); edit_task `key`; task `key`, `tab` (summary, \
  conversation or changes), `view` for what a tab opens in \
  its place (memory: the task's memory, from summary; local-integration: Local integration, which Try locally opens \
  from changes) and, on its changes tab, `file` with `from`; project_settings \
  `tab` (general, runs, context, connections, environment or sharing; secrets, the tab Environment was, still opens \
  it) and, with tab runs, `view` rules (Approved for every run). Connections and Environment are tabs of Project \
  settings with routes of their own: connections takes `selected` (the id of the connection whose page it opens); \
  secrets opens the Environment tab, the project's variables and secrets; new_connection takes `kind` (Add \
  provider or Add MCP server); edit_connection `connection` (an MCP server's id, or an agent CLI connection's for its \
  account); each opens Project settings on that tab or page. agents, instructions, skills, tools and memory are the \
  five tabs of one Agents page (Profiles, Instructions, Skills, Tools, Memory), each its own route: agents takes \
  `agent` (a profile's id, or new), skills takes \
  `skill` (a skill id, new or import), tools takes tool (a tool id, or new), and its page's tab; merge_queue and \
  pull_requests are the two tabs of one Review page, each its own route; orchestrator takes `chat` (a \
  conversation's id, or new; left out, the latest); files takes `file` (a path in the project; the tree alone when \
  left out), `at` (checkout, the default, base, run:<run id> or rev:<commit>) and `line` (12 or 12-20); pull_requests \
  takes `tab`, or `number` for one pull request (with `tab` feedback, its Agent feedback); usage takes `query`, the \
  page's query as a link carries it, and \
  without `project` shows every project's usage; with `view` delivery and `period` a project's Delivery view beside \
  it; modules takes `tab` (open or closed), `view` new for New module, or `module`: new for New module, a module's id \
  for its page on `tab` (summary or burndown), with `view` edit for Edit module (a `tab` or `view` that names nothing \
  there is refused); new_task takes `module`, a module's id, for New task in module; pipelines opens the list, from \
  whose rows a pipeline's page opens on its Steps, Settings and Versions tabs. A tab or kind left out is the one the \
  screen opens on. \
  Answers the \
  route the tab shows, whose params navigate takes back. Errors are JSON with \
  a stable `code`: unknown_route (with the known names), missing_param, unexpected_param, invalid_param, \
  unknown_tab (with the screen's tabs), not_a_string, not_an_object, not_ready (the window is starting).";

/// The tool on `link`; `lookup` finds the accounts and team servers Settings' pages are named by.
pub fn tool(link: WindowLink, lookup: Option<Lookup>) -> McpTool {
  McpTool::new("navigate")
    .description(DESCRIPTION)
    .scope(Scope::Interact)
    .input_schema(schema())
    .handler(move |_ctx, args| {
      reply(
        link
          .navigation()
          .and_then(|navigation| navigate_with(&navigation, &args, lookup.as_ref())),
      )
    })
}

fn schema() -> Value {
  let names: Vec<&str> = Screen::ALL.into_iter().map(Screen::name).collect();
  let tabs = format!(
    "The screen's tab, the one it opens on when left out: settings automation, general, updates, whats-new, \n     notifications, accounts, browser, \n     orchester (Orchester Cloud), connect (Connect a server), invite (an invite's landing); overview {} (kept for old \n     links: the Overview shows every section on one page); tasks {}; \n     task {}; project_settings {}; pull_requests {}; memory {}; import_resources {}; modules {}, a module's page {}.",
    OverviewTab::ALL.map(OverviewTab::name).join(", "),
    TaskListTab::ALL.map(TaskListTab::name).join(", "),
    TaskTab::ALL.map(TaskTab::name).join(", "),
    SETTINGS_TABS.join(", "),
    PullRequestTab::ALL.map(PullRequestTab::name).join(", "),
    MemoryTab::ALL.map(MemoryTab::name).join(", "),
    ImportTab::ALL.map(ImportTab::name).join(", "),
    ModuleListTab::ALL.map(ModuleListTab::name).join(", "),
    ModuleTab::ALL.map(ModuleTab::name).join(", "),
  );
  let kinds: Vec<&str> = NewConnection::ALL.into_iter().map(NewConnection::name).collect();
  let sources: Vec<&str> = ChangeSource::ALL.into_iter().map(ChangeSource::name).collect();
  let views: Vec<&str> = ["usage", "delivery", "rules", "edit", "new"]
    .into_iter()
    .chain(TaskView::SUB_VIEWS)
    .collect();
  let mut schema = json!({
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
      "selected": { "type": "string", "description": "connections: the id of the connection whose page Project settings Ã¢â‚¬Âº Connections opens." },
      "kind": { "type": "string", "enum": kinds, "description": "new_connection: what it adds, on a page of Project settings Ã¢â‚¬Âº Connections." },
      "connection": { "type": "string", "description": "edit_connection: the connection's id: an MCP server's form, or an agent CLI connection's account, on a page of Project settings Ã¢â‚¬Âº Connections." },
      "agent": { "type": "string", "description": "agents: a profile's id for its page, or new; omitted opens the list." },
      "skill": { "type": "string", "description": "skills: the skill id, new, or import; omitted opens the library." },
      "tool": { "type": "string", "description": "tools: the toolÃ¢â‚¬â„¢s id, or new for a command draft; omitted opens the library." },
      "chat": { "type": "string", "description": "orchestrator: a conversation's id, or new; omitted opens the latest." },
      "way": { "type": "string", "enum": ["cloud", "code", "projects", "folder"], "description": "join_team: the account's teams and invites (cloud, the default), the code page, a joined team's shared projects (projects, with `server`) or linking one of them to a folder (folder, with `server` and `project`, its slug)." },
      "at": { "type": "string", "description": "files: the version, checkout (the default), base, run:<run id> or rev:<commit>." },
      "line": { "type": "string", "description": "files: the line or lines to show, 12 or 12-20." },
      "number": { "type": "string", "description": "pull_requests: the number of the pull request to open, e.g. 142; with tab feedback, on its Agent feedback." },
      "module": { "type": "string", "description": "modules: a module's id for its page, or new for New module; omitted opens the list. new_task: a module's id, for New task in module." },
      "query": {
        "type": "string",
        "description": "usage: the period, grouping and filters as a link's query carries them, e.g. `from=2026-10-01&to=2026-10-15&group=task&model=gpt-5` (get_usage's `link`); omitted, the last 7 days by day.",
      },
      "view": {
        "type": "string",
        "enum": views,
        "description": "usage of a project: the Usage page (the default) or the Delivery view beside it. task: what a tab opens in its place, memory (the task's memory, on summary) or local-integration (Local integration, on changes); without `tab` it opens its own. project_settings with tab runs: rules, the page Approved for every run. modules: new, without `module`, opens New module; edit, with a module's id, opens Edit module.",
      },
      "period": {
        "type": "string",
        "enum": DeliveryPeriod::ALL.map(DeliveryPeriod::name),
        "description": "usage with view delivery: the days the Delivery view shows; omitted, 30d.",
      },
      "from": {
        "type": "string",
        "enum": sources,
        "description": "With `file`: its change at the run's latest checkpoint (the default) or in its worktree after it.",
      },
    },
    "required": ["route"],
    "additionalProperties": false,
  });
  if let Some(properties) = schema["properties"].as_object_mut() {
    properties.extend(settings_route::properties());
  }
  schema
}

/// Takes the active tab to the route `args` name and answers the route it shows.
pub fn navigate(navigation: &Navigation, args: &Value) -> Result<Value, ToolError> {
  navigate_with(navigation, args, None)
}

/// [`navigate`], finding the accounts and team servers Settings' pages are named by in `lookup`.
pub fn navigate_with(navigation: &Navigation, args: &Value, lookup: Option<&Lookup>) -> Result<Value, ToolError> {
  let route = target(&object(args)?, lookup)?;
  navigation.go(route);
  Ok(json!({ "route": route_json(&navigation.route_now()) }))
}

/// The route `args` name: `route`, and the parameters its screen takes.
fn target(args: &Map<String, Value>, lookup: Option<&Lookup>) -> Result<Route, ToolError> {
  let name = string(args, "route")?.ok_or(ToolError::Missing { param: "route" })?;
  let screen = Screen::from_name(name).ok_or_else(|| ToolError::UnknownRoute { name: name.to_owned() })?;
  only_params(screen, args)?;
  screen_route(screen, args, lookup)
}

/// A validated screen and its parameters as the window's route.
fn screen_route(screen: Screen, args: &Map<String, Value>, lookup: Option<&Lookup>) -> Result<Route, ToolError> {
  let page = match screen {
    Screen::AllProjects => return Ok(Route::AllProjects),
    Screen::AddProject => return Ok(Route::AddProject(None)),
    Screen::JoinTeam => return join_team::route(args, lookup),
    Screen::FirstDownload => return Ok(Route::FirstDownload(project(args)?)),
    Screen::Settings => return settings_route::route(args, lookup),
    Screen::Overview => ProjectPage::Overview(tab(
      args,
      OverviewTab::from_name,
      &OverviewTab::ALL.map(OverviewTab::name),
    )?),
    Screen::Orchestrator => ProjectPage::Orchestrator(orchestrator_chat(args)?),
    Screen::OrchestratorSettings => ProjectPage::OrchestratorSettings,
    Screen::Tasks => ProjectPage::Tasks(task_list(args)?),
    Screen::NewTask => new_task_page(args)?,
    Screen::EditTask => ProjectPage::EditTask(key(args, "key")?),
    Screen::Task => ProjectPage::Task(key(args, "key")?, task_view(args)?),
    Screen::Modules => module_page(args)?,
    Screen::MergeQueue => ProjectPage::MergeQueue,
    Screen::PullRequests => pull_requests_page(args)?,
    Screen::Usage => return usage_route(args),
    Screen::Pipelines => ProjectPage::Pipelines,
    Screen::Agents => agent_page(args)?,
    Screen::Instructions => ProjectPage::Instructions(InstructionsView::default()),
    Screen::Memory => ProjectPage::Memory(memory_tab(args)?),
    Screen::Skills => skills_page(args)?,
    Screen::Tools => tools_page(args)?,
    Screen::Secrets => ProjectPage::Secrets(SecretsView::default()),
    Screen::Connections => ProjectPage::Connections(connections_view(args)?),
    Screen::NewConnection => ProjectPage::NewConnection(new_connection(args)?),
    Screen::EditConnection => ProjectPage::EditConnection(connection(args, "connection")?),
    Screen::ProjectSettings => project_settings_page(args)?,
    Screen::Files => ProjectPage::Files(files_view(args)?),
    Screen::ImportResources => return import_route(args),
  };
  Ok(Route::Project(project(args)?, page))
}

/// Usage of the project `project` names, or everywhere when it is left out.
fn usage_route(args: &Map<String, Value>) -> Result<Route, ToolError> {
  match string(args, "project")? {
    Some(_) => Ok(Route::Project(project(args)?, usage_page(args)?)),
    None => usage_everywhere(args),
  }
}

/// Refuses a parameter besides `route` that `screen` does not take.
fn only_params(screen: Screen, args: &Map<String, Value>) -> Result<(), ToolError> {
  let takes = params(screen);
  match args
    .keys()
    .find(|key| *key != "route" && !takes.contains(&key.as_str()))
  {
    Some(param) => Err(ToolError::UnexpectedParam {
      of: screen.name(),
      param: param.clone(),
    }),
    None => Ok(()),
  }
}

/// Import resources of `project` on the kind tab `tab` names, or All.
fn import_route(args: &Map<String, Value>) -> Result<Route, ToolError> {
  let names = ImportTab::ALL.map(ImportTab::name);
  let tab = tab(args, ImportTab::from_name, &names)?;
  Ok(Route::Project(project(args)?, ProjectPage::Import(tab)))
}

/// The parameters besides `route` that `screen` takes.
const fn params(screen: Screen) -> &'static [&'static str] {
  match screen {
    Screen::AllProjects | Screen::AddProject => &[],
    Screen::JoinTeam => join_team::PARAMS,
    Screen::FirstDownload => &["project"],
    Screen::Settings => settings_route::PARAMS,
    Screen::Overview | Screen::ImportResources => &["project", "tab"],
    Screen::Tasks => &["project", "tab", "q", "highlight"],
    Screen::EditTask => &["project", "key"],
    Screen::Task => &["project", "key", "tab", "view", "file", "from"],
    Screen::Connections => &["project", "selected"],
    Screen::Memory => &["project", "tab"],
    Screen::Tools => &["project", "tool", "tab"],
    Screen::Skills => &["project", "skill"],
    Screen::Orchestrator => &["project", "chat"],
    Screen::Files => &["project", "file", "at", "line"],
    Screen::PullRequests => &["project", "tab", "number"],
    Screen::NewConnection => &["project", "kind"],
    Screen::EditConnection => &["project", "connection"],
    Screen::Usage => &["project", "query", "view", "period"],
    Screen::Modules => &["project", "module", "tab", "view"],
    Screen::NewTask => &["project", "module"],
    Screen::ProjectSettings => &["project", "tab", "view"],
    Screen::Agents => &["project", "agent"],
    Screen::MergeQueue | Screen::Pipelines | Screen::Instructions | Screen::Secrets | Screen::OrchestratorSettings => {
      &["project"]
    }
  }
}

/// Memory's tab, or none when left out: the page opens on the one it chooses.
fn memory_tab(args: &Map<String, Value>) -> Result<Option<MemoryTab>, ToolError> {
  string(args, "tab")?
    .map(|name| {
      MemoryTab::from_name(name).ok_or_else(|| ToolError::UnknownTab {
        name: name.to_owned(),
        known: MemoryTab::ALL.map(MemoryTab::name).to_vec(),
      })
    })
    .transpose()
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

/// The Usage page with the query `query` gives, as its path reads it; with `view` delivery, the Delivery view beside
/// it over `period`.
fn usage_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  match string(args, "view")? {
    None | Some("usage") => {}
    Some("delivery") => return delivery_page(args),
    Some(other) => {
      return Err(ToolError::InvalidParam {
        param: "view",
        reason: format!("`{other}` is neither usage nor delivery"),
      });
    }
  }
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

/// The Delivery view over `period` (30d when left out).
fn delivery_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let Some(name) = string(args, "period")? else {
    return Ok(ProjectPage::Delivery(DeliveryPeriod::default()));
  };
  DeliveryPeriod::from_name(name)
    .map(ProjectPage::Delivery)
    .ok_or_else(|| ToolError::InvalidParam {
      param: "period",
      reason: format!("`{name}` is not 30d, 90d or 365d"),
    })
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
pub(super) fn tab<T: Default>(
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
pub(super) fn connection(args: &Map<String, Value>, param: &'static str) -> Result<ConnectionId, ToolError> {
  let value = string(args, param)?.ok_or(ToolError::Missing { param })?;
  value.parse().map_err(|error: uuid::Error| ToolError::InvalidParam {
    param,
    reason: format!("`{value}` is not a connection id ({error})"),
  })
}

/// Pull request `number` on Files changed, or on Agent feedback with `tab` `feedback`; or the list on `tab`.
fn pull_requests_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let Some(number) = string(args, "number")? else {
    let known = PullRequestTab::ALL.map(PullRequestTab::name);
    return Ok(ProjectPage::PullRequests(tab(args, PullRequestTab::from_name, &known)?));
  };
  let feedback = match string(args, "tab")? {
    None => false,
    Some("feedback") => true,
    Some(_) => {
      return Err(ToolError::InvalidParam {
        param: "tab",
        reason: "a pull request's own tabs are files (no tab) and feedback".to_owned(),
      });
    }
  };
  let number = number
    .trim_start_matches('#')
    .parse::<NonZeroU64>()
    .map_err(|_| ToolError::InvalidParam {
      param: "number",
      reason: format!("`{number}` is not a pull request number"),
    })?;
  Ok(if feedback {
    ProjectPage::PullRequestFeedback(number, None)
  } else {
    ProjectPage::PullRequest(number, None)
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

/// The task's tab, the view `view` names in its place, and on Changes the file `file` and `from` name; a file alone
/// opens Changes.
fn task_view(args: &Map<String, Value>) -> Result<TaskView, ToolError> {
  let (file, from) = (string(args, "file")?, string(args, "from")?);
  let tab = tab(args, TaskTab::from_name, &TaskTab::ALL.map(TaskTab::name))?;
  if let Some(view) = string(args, "view")? {
    return task_sub_view(args, tab, view);
  }
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

/// The view `name` opens in place of its tab: memory under summary, local-integration under changes. Left without a
/// tab, the view opens its own; a file goes with no view.
fn task_sub_view(args: &Map<String, Value>, tab: TaskTab, name: &str) -> Result<TaskView, ToolError> {
  if args.contains_key("file") {
    return Err(ToolError::InvalidParam {
      param: "view",
      reason: "a file is shown on the changes tab itself, not in a view".to_owned(),
    });
  }
  let tabs = if args.contains_key("tab") {
    vec![tab]
  } else {
    TaskTab::ALL.to_vec()
  };
  tabs
    .into_iter()
    .find_map(|tab| TaskView::sub_view(tab, name))
    .ok_or_else(|| ToolError::InvalidParam {
      param: "view",
      reason: format!(
        "`{name}` names no view there: the task's views are memory, on summary, and local-integration, on changes"
      ),
    })
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
    None => Ok(ProjectPage::Skills(SkillsFilter::default())),
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
