use lurq::mcp::{McpTool, Scope};
use orchester_core::{
  id::ProjectId,
  project::{TaskKey, TaskKeyError},
};
use serde_json::{Map, Value, json};

use super::{ToolError, WindowLink, object, reply, route_json, string};
use crate::{
  navigation::Navigation,
  routes::{
    ChangeSource, ChangedFile, OverviewTab, ProjectPage, Route, Screen, TaskList, TaskListTab, TaskTab, TaskView,
  },
};

const DESCRIPTION: &str = "Take the active tab of the Orchester window to a route, as a new entry of its history \
  (the mouse's back button, or Alt+Left, Cmd+[ on macOS, returns). `route` is the screen's name: all_projects and \
  settings take no other argument; every other screen takes `project`, the project's id (a UUID, as app_state \
  reports it). overview also takes `tab`; tasks `tab`, `q` (the search) and `highlight` (a task key, as after \
  Create); edit_task `key`; task `key`, `tab` and, on its changes tab, `file` with `from`. A tab left out is the \
  one the screen opens on. Answers the route the tab shows, whose params navigate takes back. Errors are JSON with \
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
    "The screen's tab, the one it opens on when left out: overview {}; tasks {}; task {}.",
    OverviewTab::ALL.map(OverviewTab::name).join(", "),
    TaskListTab::ALL.map(TaskListTab::name).join(", "),
    TaskTab::ALL.map(TaskTab::name).join(", "),
  );
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
        "description": "task: the repository path of a changed file to show on the changes tab (the tab when left out).",
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
    Screen::Settings => return Ok(Route::Settings),
    Screen::Overview => ProjectPage::Overview(tab(args, &OverviewTab::ALL, OverviewTab::name)?),
    Screen::Tasks => ProjectPage::Tasks(task_list(args)?),
    Screen::EditTask => ProjectPage::EditTask(key(args, "key")?),
    Screen::Task => ProjectPage::Task(key(args, "key")?, task_view(args)?),
    screen => ProjectPage::of(screen).expect("every other screen names nothing but its project"),
  };
  Ok(Route::Project(project(args)?, page))
}

/// The parameters besides `route` that `screen` takes.
const fn params(screen: Screen) -> &'static [&'static str] {
  match screen {
    Screen::AllProjects | Screen::Settings => &[],
    Screen::Overview => &["project", "tab"],
    Screen::Tasks => &["project", "tab", "q", "highlight"],
    Screen::EditTask => &["project", "key"],
    Screen::Task => &["project", "key", "tab", "file", "from"],
    _ => &["project"],
  }
}

fn project(args: &Map<String, Value>) -> Result<ProjectId, ToolError> {
  let value = string(args, "project")?.ok_or(ToolError::Missing { param: "project" })?;
  value.parse().map_err(|error: uuid::Error| ToolError::InvalidParam {
    param: "project",
    reason: error.to_string(),
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

/// The tab of `all` that `tab` names by its `name`, or the one the screen opens on when it names none.
fn tab<T: Copy + Default>(args: &Map<String, Value>, all: &[T], name: fn(T) -> &'static str) -> Result<T, ToolError> {
  let Some(given) = string(args, "tab")? else {
    return Ok(T::default());
  };
  all
    .iter()
    .copied()
    .find(|tab| name(*tab) == given)
    .ok_or_else(|| ToolError::UnknownTab {
      name: given.to_owned(),
      known: all.iter().copied().map(name).collect(),
    })
}

fn task_list(args: &Map<String, Value>) -> Result<TaskList, ToolError> {
  let mut list = TaskList::new(tab(args, &TaskListTab::ALL, TaskListTab::name)?);
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
  let Some(path) = file else {
    if from.is_some() {
      return Err(ToolError::InvalidParam {
        param: "from",
        reason: "it says where the change of `file` is read from; give `file` too".to_owned(),
      });
    }
    return Ok(TaskView::of(tab(args, &TaskTab::ALL, TaskTab::name)?));
  };
  if string(args, "tab")?.is_some() && tab(args, &TaskTab::ALL, TaskTab::name)? != TaskTab::Changes {
    return Err(ToolError::InvalidParam {
      param: "file",
      reason: "a file is shown on the changes tab".to_owned(),
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
  let file = ChangedFile::new(path, source).map_err(|error| ToolError::InvalidParam {
    param: "file",
    reason: error.to_string(),
  })?;
  Ok(TaskView::Changes(Some(file)))
}
