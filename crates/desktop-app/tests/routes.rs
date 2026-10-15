//! Every route has a path, query included, that reads back as the same route and that the router's table
//! resolves to its screen; a path that names nothing reads as a typed error that says why, and falls back
//! in the table.

use chrono::NaiveDate;
use orchester_app::routes::{
  ChangeSource, ChangedFile, ConnectionsTab, ConnectionsView, NewConnection, OverviewTab, ProjectPage, Route,
  RouteError, RouteFolder, Screen, TaskList, TaskListTab, TaskTab, TaskView, routes,
};
use orchester_core::{
  id::{ConnectionId, ProjectId},
  project::TaskKey,
  usage::{DayRange, GroupBy, Period},
};
use orchester_workspace::usage::{UsageQuery, usage_link};

const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";
const CONNECTION: &str = "7e1f2a3b-4c5d-4e6f-8a9b-0c1d2e3f4a5b";
/// A search and a file path with every character a query must escape, and some it need not.
const AWKWARD: &str = "retry & jitter #2 ? 100% = ü/слово+";

fn project() -> ProjectId {
  PROJECT.parse().expect("a project id")
}

fn connection() -> ConnectionId {
  CONNECTION.parse().expect("a connection id")
}

fn key(key: &str) -> TaskKey {
  key.parse().expect("a task key")
}

fn at(page: ProjectPage) -> Route {
  Route::Project(project(), page)
}

fn file(path: &str, source: ChangeSource) -> ChangedFile {
  ChangedFile::new(path, source).expect("a file path")
}

/// One route of every screen, and every tab, query and file a route can name.
fn every_route() -> Vec<Route> {
  let mut all = vec![
    Route::AllProjects,
    Route::AddProject(None),
    Route::AddProject(RouteFolder::new(std::path::Path::new(AWKWARD))),
    Route::Settings,
  ];
  all.extend(Screen::ALL.into_iter().filter_map(ProjectPage::of).map(at));
  all.extend(OverviewTab::ALL.map(|tab| at(ProjectPage::Overview(tab))));
  all.extend(TaskListTab::ALL.map(|tab| at(ProjectPage::Tasks(TaskList::new(tab)))));
  all.extend([
    at(ProjectPage::Tasks(TaskList::default().with_search("retry"))),
    at(ProjectPage::Tasks(TaskList::default().with_search(AWKWARD))),
    at(ProjectPage::Tasks(TaskList::default().with_highlight(key("KON-13")))),
    at(ProjectPage::Tasks(
      TaskList::new(TaskListTab::InReview)
        .with_search("  spaced  ")
        .with_highlight(key("KON-13")),
    )),
    at(ProjectPage::NewSkill),
    at(ProjectPage::Skill(PROJECT.parse().expect("a skill id"))),
    at(ProjectPage::ImportSkills),
    at(ProjectPage::NewPipeline),
    at(ProjectPage::NewEmptyPipeline),
    at(ProjectPage::Pipeline(PROJECT.parse().expect("a resource id"))),
    at(ProjectPage::Agent(CONNECTION.parse().expect("an agent resource id"))),
    at(ProjectPage::EditTask(key("KON-7"))),
    at(ProjectPage::Usage(Box::new(usage_query()))),
    at(ProjectPage::EditConnection(connection())),
    at(ProjectPage::Connections(
      ConnectionsView::new(ConnectionsTab::McpServers).with_selected(connection()),
    )),
  ]);
  all.extend(ConnectionsTab::ALL.map(|tab| at(ProjectPage::Connections(ConnectionsView::new(tab)))));
  all.extend(NewConnection::ALL.map(|kind| at(ProjectPage::NewConnection(kind))));
  all.extend(TaskTab::ALL.map(|tab| at(ProjectPage::Task(key("KON-12"), TaskView::of(tab)))));
  for source in ChangeSource::ALL {
    for path in ["src/webhooks/sender.rs", AWKWARD] {
      let view = TaskView::Changes(Some(file(path, source)));
      all.push(at(ProjectPage::Task(key("KON-12"), view)));
    }
  }
  all
}

#[test]
fn every_screen_has_a_route() {
  let screens: Vec<Screen> = every_route().iter().map(Route::screen).collect();
  for screen in Screen::ALL {
    assert!(screens.contains(&screen), "{screen:?} has no route");
  }
}

#[test]
fn every_route_reads_back_from_its_path() {
  for route in every_route() {
    assert_eq!(Route::parse(&route.path()), Ok(route.clone()), "{}", route.path());
  }
}

#[test]
fn paths_are_the_documented_ones() {
  let root = format!("/project/{PROJECT}");
  let paths = [
    (Route::AllProjects, "/".to_owned()),
    (Route::AddProject(None), "/add-project".to_owned()),
    (
      Route::AddProject(RouteFolder::new(std::path::Path::new("C:/work/a b"))),
      "/add-project?folder=C%3A/work/a%20b".to_owned(),
    ),
    (Route::Settings, "/settings".to_owned()),
    (at(ProjectPage::Overview(OverviewTab::Running)), root.clone()),
    (
      at(ProjectPage::Overview(OverviewTab::NeedsYou)),
      format!("{root}?tab=needs_you"),
    ),
    (at(ProjectPage::Tasks(TaskList::default())), format!("{root}/tasks")),
    (
      at(ProjectPage::Tasks(
        TaskList::new(TaskListTab::InProgress)
          .with_search("retry jitter")
          .with_highlight(key("KON-13")),
      )),
      format!("{root}/tasks?tab=in_progress&q=retry%20jitter&highlight=KON-13"),
    ),
    (at(ProjectPage::NewTask), format!("{root}/tasks/new")),
    (
      at(ProjectPage::EditTask(key("KON-7"))),
      format!("{root}/tasks/KON-7/edit"),
    ),
    (
      at(ProjectPage::Task(key("KON-12"), TaskView::Summary)),
      format!("{root}/tasks/KON-12/summary"),
    ),
    (
      at(ProjectPage::Task(key("KON-12"), TaskView::Conversation)),
      format!("{root}/tasks/KON-12/conversation"),
    ),
    (
      at(ProjectPage::Task(key("KON-12"), TaskView::Changes(None))),
      format!("{root}/tasks/KON-12/changes"),
    ),
    (
      at(ProjectPage::Task(
        key("KON-12"),
        TaskView::Changes(Some(file("src/a b.rs", ChangeSource::Checkpoint))),
      )),
      format!("{root}/tasks/KON-12/changes?file=src/a%20b.rs"),
    ),
    (
      at(ProjectPage::Task(
        key("KON-12"),
        TaskView::Changes(Some(file("src/a&b.rs", ChangeSource::Worktree))),
      )),
      format!("{root}/tasks/KON-12/changes?file=src/a%26b.rs&from=worktree"),
    ),
    (at(ProjectPage::PullRequests), format!("{root}/pull-requests")),
    (at(ProjectPage::Settings), format!("{root}/settings")),
  ];
  for (route, path) in paths {
    assert_eq!(route.path(), path);
  }
}

#[test]
fn the_router_table_resolves_every_route_to_its_screen() {
  let table = routes();
  for route in every_route() {
    let path = route.path();
    let chain = table.resolve(&path);
    let leaf = chain.last().unwrap_or_else(|| panic!("{path} resolves"));
    assert_eq!(leaf.pattern_raw(), route.pattern(), "{path}");
  }
}

#[test]
fn a_tasks_tabs_share_one_router_pattern_and_its_forms_keep_their_own() {
  let table = routes();
  let leaf = |path: String| {
    let chain = table.resolve(&path);
    chain
      .last()
      .unwrap_or_else(|| panic!("{path} resolves"))
      .pattern_raw()
      .to_owned()
  };
  let task = format!("/project/{PROJECT}/tasks");
  for tab in TaskTab::ALL {
    let path = at(ProjectPage::Task(key("KON-12"), TaskView::of(tab))).path();
    assert_eq!(
      leaf(path),
      Screen::Task.pattern(),
      "switching tabs keeps the task's page"
    );
  }
  assert_eq!(leaf(format!("{task}/KON-12/edit")), Screen::EditTask.pattern());
  assert_eq!(leaf(format!("{task}/new")), Screen::NewTask.pattern());
  assert_eq!(TaskTab::default(), TaskTab::Summary, "a task opens on its Summary");
  assert_eq!(TaskTab::ALL[0], TaskTab::Summary, "in the design's order");
}

#[test]
fn paths_that_name_no_screen_are_no_screen_and_fall_back_in_the_router() {
  for path in [
    "",
    "settings",
    "/nope",
    "/settings/general",
    "/project",
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/nope",
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/tasks/KON-12/nope",
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/tasks/KON-12/summary/more",
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/tasks/KON-12/edit/more",
    "/nope?q=%zz",
    "/settings/",
    "//",
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/tasks/new/",
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b//tasks",
    "/project/not-an-id/",
  ] {
    assert_eq!(Route::parse(path), Err(RouteError::NoScreen), "{path:?}");
  }
  let table = routes();
  let screens: Vec<String> = Screen::ALL.into_iter().map(Screen::pattern).collect();
  for path in [
    "/nope",
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/nope?tab=all",
    "/settings/general",
  ] {
    let chain = table.resolve(path);
    let leaf = chain.last().unwrap_or_else(|| panic!("{path} falls back"));
    assert!(
      !screens.iter().any(|pattern| pattern == leaf.pattern_raw()),
      "{path} names no screen"
    );
  }
}

#[test]
fn paths_name_projects_by_id_and_tasks_by_key() {
  let project_id = |value: &str| RouteError::ProjectId {
    value: value.to_owned(),
  };
  assert_eq!(Route::parse("/project/kontur"), Err(project_id("kontur")));
  assert_eq!(Route::parse("/project/../tasks"), Err(project_id("..")));
  for task in ["KON-0", "kon-12", "new-task"] {
    let error = Route::parse(&format!("/project/{PROJECT}/tasks/{task}/summary")).expect_err("not a task key");
    assert!(
      matches!(&error, RouteError::TaskKey { value, .. } if value == task),
      "{task}: {error:?}"
    );
  }
  let error = Route::parse(&format!("/project/{PROJECT}/tasks/KON-0/edit")).expect_err("not a task key");
  assert!(matches!(error, RouteError::TaskKey { .. }), "{error:?}");
}

#[test]
fn a_query_holds_only_what_its_screen_takes() {
  let unexpected = |param: &str| RouteError::UnexpectedParam {
    param: param.to_owned(),
  };
  let invalid = |param: &str, value: &str| RouteError::InvalidParam {
    param: param.to_owned(),
    value: value.to_owned(),
  };
  let task = format!("/project/{PROJECT}/tasks/KON-12");
  let refusals = [
    ("/?tab=all".to_owned(), unexpected("tab")),
    (format!("/project/{PROJECT}/usage?tab=all"), unexpected("tab")),
    (format!("/project/{PROJECT}/tasks/new?q=x"), unexpected("q")),
    (format!("/project/{PROJECT}/tasks?tab=all&tab=done"), unexpected("tab")),
    (format!("{task}/summary?file=a.rs"), unexpected("file")),
    (format!("{task}/changes?from=worktree"), unexpected("from")),
    (format!("/project/{PROJECT}?tab=all"), invalid("tab", "all")),
    (format!("/project/{PROJECT}?tab="), invalid("tab", "")),
    (
      format!("/project/{PROJECT}/tasks?tab=running"),
      invalid("tab", "running"),
    ),
    (
      format!("/project/{PROJECT}/tasks?highlight=kon-1"),
      invalid("highlight", "kon-1"),
    ),
    (format!("/project/{PROJECT}/tasks?q=%zz"), invalid("q", "%zz")),
    (format!("/project/{PROJECT}/tasks?q=%2"), invalid("q", "%2")),
    (format!("{task}/changes?file="), invalid("file", "")),
    (
      format!("{task}/changes?file=a.rs&from=nowhere"),
      invalid("from", "nowhere"),
    ),
  ];
  for (path, error) in refusals {
    assert_eq!(Route::parse(&path), Err(error), "{path}");
  }
}

#[test]
fn an_empty_search_is_no_search_and_a_changed_file_has_a_path() {
  assert_eq!(TaskList::default().with_search(""), TaskList::default());
  assert_eq!(
    Route::parse(&format!("/project/{PROJECT}/tasks?q=")),
    Ok(at(ProjectPage::Tasks(TaskList::default())))
  );
  assert_eq!(
    TaskList::default().with_search(" ").search(),
    Some(" "),
    "kept as typed"
  );
  assert!(ChangedFile::new("", ChangeSource::Worktree).is_err());
  let changes = TaskView::Changes(Some(file("a.rs", ChangeSource::Worktree)));
  assert_eq!(changes.tab(), TaskTab::Changes);
  assert_eq!(changes.file().map(ChangedFile::path), Some("a.rs"));
  assert_eq!(TaskView::Summary.file(), None);
}

#[test]
fn screens_open_on_their_first_tab() {
  assert_eq!(
    ProjectPage::of(Screen::Overview),
    Some(ProjectPage::Overview(OverviewTab::Running))
  );
  assert_eq!(
    ProjectPage::of(Screen::Tasks),
    Some(ProjectPage::Tasks(TaskList::new(TaskListTab::All)))
  );
  assert_eq!(ProjectPage::of(Screen::NewTask), Some(ProjectPage::NewTask));
  for screen in [Screen::AllProjects, Screen::Settings, Screen::Task, Screen::EditTask] {
    assert_eq!(ProjectPage::of(screen), None, "{screen:?}");
  }
  assert_eq!(TaskView::default(), TaskView::Summary);
}

#[test]
fn screens_tabs_and_sources_read_back_from_their_names() {
  for screen in Screen::ALL {
    assert_eq!(Screen::from_name(screen.name()), Some(screen));
  }
  for tab in TaskTab::ALL {
    assert_eq!(TaskTab::from_name(tab.name()), Some(tab));
  }
  for tab in TaskListTab::ALL {
    assert_eq!(TaskListTab::from_name(tab.name()), Some(tab));
  }
  for tab in OverviewTab::ALL {
    assert_eq!(OverviewTab::from_name(tab.name()), Some(tab));
  }
  for source in ChangeSource::ALL {
    assert_eq!(ChangeSource::from_name(source.name()), Some(source));
  }
  assert_eq!(Screen::from_name("dashboard"), None);
}

#[test]
fn a_task_and_its_forms_select_tasks_in_the_sidebar() {
  for page in [
    ProjectPage::Task(key("KON-12"), TaskView::Summary),
    ProjectPage::NewTask,
    ProjectPage::EditTask(key("KON-7")),
  ] {
    let route = at(page);
    assert_eq!(route.nav_screen(), Screen::Tasks, "{route:?}");
    assert_eq!(route.project(), Some(project()));
  }
  assert_eq!(at(ProjectPage::NewTask).screen(), Screen::NewTask);
  assert_eq!(Route::Settings.project(), None);
}

#[test]
fn connections_pages_have_their_documented_paths_and_select_connections() {
  let root = format!("/project/{PROJECT}/connections");
  for (page, path) in [
    (ProjectPage::Connections(ConnectionsView::default()), root.clone()),
    (
      ProjectPage::Connections(ConnectionsView::new(ConnectionsTab::McpServers).with_selected(connection())),
      format!("{root}?tab=mcp_servers&selected={CONNECTION}"),
    ),
    (
      ProjectPage::NewConnection(NewConnection::AgentProvider),
      format!("{root}/new"),
    ),
    (
      ProjectPage::NewConnection(NewConnection::McpServer),
      format!("{root}/new?kind=mcp_server"),
    ),
    (
      ProjectPage::EditConnection(connection()),
      format!("{root}/{CONNECTION}/edit"),
    ),
  ] {
    let route = at(page);
    assert_eq!(route.path(), path);
    assert_eq!(route.nav_screen(), Screen::Connections, "{route:?}");
  }
  assert_eq!(
    Route::parse(&format!("{root}/not-an-id/edit")),
    Err(RouteError::NoScreen)
  );
  assert!(matches!(
    Route::parse(&format!("{root}?selected=nope")),
    Err(RouteError::InvalidParam { .. })
  ));
}
#[test]
fn pipelines_pages_have_their_own_paths_and_select_pipelines() {
  let id = PROJECT.parse().expect("resource id");
  for (page, suffix) in [
    (ProjectPage::NewPipeline, "new".to_owned()),
    (ProjectPage::Pipeline(id), id.to_string()),
  ] {
    let route = at(page);
    assert_eq!(route.path(), format!("/project/{PROJECT}/pipelines/{suffix}"));
    assert_eq!(Route::parse(&route.path()), Ok(route.clone()));
    assert_eq!(route.screen(), Screen::Pipelines);
  }
}

#[test]
fn agents_pages_have_exact_current_resource_paths_and_select_agents() {
  let id = CONNECTION.parse().expect("a resource id");
  let route = at(ProjectPage::Agent(id));
  assert_eq!(route.path(), format!("/project/{PROJECT}/agents/{CONNECTION}"));
  assert_eq!(Route::parse(&route.path()), Ok(route.clone()));
  assert_eq!(route.nav_screen(), Screen::Agents);
  assert_eq!(route.pattern(), "/project/:project/agents/:agent");
  let table = routes();
  let chain = table.resolve(&route.path());
  assert_eq!(
    chain.last().expect("agent route resolves").pattern_raw(),
    route.pattern()
  );
  assert_eq!(at(ProjectPage::Agents).path(), format!("/project/{PROJECT}/agents"));
  assert_eq!(
    Route::parse(&format!("/project/{PROJECT}/agents/not-an-id")),
    Err(RouteError::NoScreen)
  );
}

/// The Usage page over a custom range by agent, filtered to a model with a space and an ampersand.
fn usage_query() -> UsageQuery {
  let range = DayRange::new(
    NaiveDate::from_ymd_opt(2026, 10, 1).expect("a day"),
    NaiveDate::from_ymd_opt(2026, 10, 15).expect("a day"),
  )
  .expect("a range");
  let mut query = UsageQuery {
    period: Period::Custom(range),
    group: GroupBy::Agent,
    ..UsageQuery::default()
  };
  query.filter.models.push(AWKWARD.into());
  query.filter.tasks.push("KON-12".into());
  query
}

#[test]
fn usage_pages_carry_their_query_as_the_workspace_links_it() {
  let query = usage_query();
  let route = at(ProjectPage::Usage(Box::new(query.clone())));
  assert_eq!(
    route.path(),
    usage_link(project(), &query),
    "the router writes the link the tools return"
  );
  assert_eq!(Route::parse(&route.path()), Ok(route.clone()));
  assert_eq!(route.nav_screen(), Screen::Usage);
  assert_eq!(
    at(ProjectPage::Usage(Box::default())).path(),
    format!("/project/{PROJECT}/usage")
  );
  let table = routes();
  let chain = table.resolve(&route.path());
  assert_eq!(
    chain.last().expect("the usage route resolves").pattern_raw(),
    route.pattern()
  );
  for (query, param) in [
    ("period=week", "period"),
    ("group=month", "group"),
    ("from=2026-10-01", "from"),
    ("from=2025-01-01&to=2026-01-02", "to"),
    ("model=", "model"),
  ] {
    assert!(
      matches!(
        Route::parse(&format!("/project/{PROJECT}/usage?{query}")),
        Err(RouteError::InvalidParam { param: given, .. }) if given == param
      ),
      "{query}"
    );
  }
  let Ok(Route::Project(_, ProjectPage::Usage(today))) =
    Route::parse(&format!("/project/{PROJECT}/usage?period=today"))
  else {
    panic!("today's usage is a route");
  };
  assert_eq!(
    today.group,
    GroupBy::Hour,
    "today reads by hour, as pressing Today shows it"
  );
  let everywhere = Route::Usage(Box::new(UsageQuery {
    filter: orchester_core::usage::UsageFilter {
      projects: vec![project()],
      ..Default::default()
    },
    ..usage_query()
  }));
  assert_eq!(
    Route::parse(&everywhere.path()),
    Ok(everywhere.clone()),
    "every project's usage"
  );
  assert!(everywhere.path().starts_with(&format!(
    "/usage?from=2026-10-01&to=2026-10-15&group=agent&project={PROJECT}"
  )));
  assert_eq!(everywhere.nav_screen(), Screen::AllProjects);
  assert!(
    matches!(
      Route::parse(&format!("/project/{PROJECT}/usage?project={PROJECT}")),
      Err(RouteError::InvalidParam { param, .. }) if param == "project"
    ),
    "a project's page is the project's"
  );
  assert_eq!(
    Route::parse(&format!("/project/{PROJECT}/usage?tab=table")),
    Err(RouteError::UnexpectedParam {
      param: "tab".to_owned()
    })
  );
}

#[test]
fn route_folder_keeps_a_folder_whole_or_names_none() {
  let awkward = std::path::Path::new(AWKWARD);
  let folder = RouteFolder::new(awkward).expect("a path that is text");
  assert_eq!(folder.path(), awkward);
  #[cfg(windows)]
  let not_text = {
    use std::os::windows::ffi::OsStringExt;
    std::path::PathBuf::from(std::ffi::OsString::from_wide(&[0x0043, 0xD800]))
  };
  #[cfg(unix)]
  let not_text = {
    use std::os::unix::ffi::OsStrExt;
    std::path::PathBuf::from(std::ffi::OsStr::from_bytes(&[0x43, 0xff]))
  };
  assert_eq!(
    RouteFolder::new(&not_text),
    None,
    "a path that is not text is never cut short"
  );
}
