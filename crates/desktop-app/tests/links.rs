//! Every kind of link `orchester_workspace::links` makes, the links the domain tools return
//! (`docs/design/orchestrator-toolset.md`, "Links"), reads as the route of its place, writes back as the same
//! path and resolves in the router's table to that route's screen, so a link a tool returns opens its place.
//! Pages that are not built yet (Usage, a pull request) have their routes too.

use std::num::NonZeroU64;

use orchester_app::routes::{
  AccountsPanel, AccountsView, ConnectionsView, OverviewTab, ProjectPage, Route, RouteError, TaskView, ToolTab, routes,
};
use orchester_core::{id::ProjectId, project::TaskKey};
use orchester_workspace::links::{Link, TaskPage, UsageGroup, UsageLink, UsageParam};

const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";
const ID: &str = "7e1f2a3b-4c5d-4e6f-8a9b-0c1d2e3f4a5b";

/// How many kinds of link there are: one per variant of [`Link`].
const KINDS: usize = 12;

fn project() -> ProjectId {
  PROJECT.parse().expect("a project id")
}

fn number(number: u64) -> NonZeroU64 {
  NonZeroU64::new(number).expect("a pull request number")
}

fn key() -> TaskKey {
  "ORC-24".parse().expect("a task key")
}

fn id<T: std::str::FromStr>() -> T {
  ID.parse().unwrap_or_else(|_| panic!("{ID} is an id"))
}

fn at(page: ProjectPage) -> Route {
  Route::Project(project(), page)
}

/// Usage with every filter set.
fn every_filter() -> UsageLink {
  let mut filters = UsageLink::default();
  for (param, value) in [
    (UsageParam::From, "2026-10-01"),
    (UsageParam::To, "2026-10-15"),
    (UsageParam::Group, "task"),
    (UsageParam::Task, "ORC-24"),
    (UsageParam::Run, ID),
    (UsageParam::Pipeline, ID),
    (UsageParam::Step, "implement"),
    (UsageParam::Agent, ID),
    // A model id is any text without spaces: every character a query must escape reads back.
    (UsageParam::Model, "gpt-5.1&codex#2?x=100%+y"),
    (UsageParam::Account, ID),
  ] {
    filters
      .set(param, value)
      .unwrap_or_else(|_| panic!("{value} is a {param:?}"));
  }
  filters
}

/// Which kind `link` is. A new kind fails to compile here until it is given a number and a case in
/// [`every_link`].
const fn kind(link: &Link) -> usize {
  match link {
    Link::Project(_) => 0,
    Link::Task(..) => 1,
    Link::Pipeline(..) => 2,
    Link::Profile(..) => 3,
    Link::Instructions(_) => 4,
    Link::Skill(..) => 5,
    Link::Tool(..) => 6,
    Link::Secrets(_) => 7,
    Link::Connection(..) => 8,
    Link::Account(_) => 9,
    Link::Usage(..) => 10,
    Link::PullRequest(..) => 11,
  }
}

/// Every kind of link, each tab of a task and Usage with and without its filters, with the route each names.
fn every_link() -> Vec<(Link, Route)> {
  let project = project();
  vec![
    (
      Link::Project(project),
      at(ProjectPage::Overview(OverviewTab::default())),
    ),
    (
      Link::Task(project, key(), TaskPage::Summary),
      at(ProjectPage::Task(key(), TaskView::Summary)),
    ),
    (
      Link::Task(project, key(), TaskPage::Conversation),
      at(ProjectPage::Task(key(), TaskView::Conversation)),
    ),
    (
      Link::Task(project, key(), TaskPage::Changes),
      at(ProjectPage::Task(key(), TaskView::Changes(None))),
    ),
    (Link::Pipeline(project, id()), at(ProjectPage::Pipeline(id()))),
    (Link::Profile(project, id()), at(ProjectPage::Agent(id()))),
    (Link::Instructions(project), at(ProjectPage::Instructions)),
    (Link::Skill(project, id()), at(ProjectPage::Skill(id()))),
    (
      Link::Tool(project, id()),
      at(ProjectPage::Tool(id(), ToolTab::Configuration)),
    ),
    (Link::Secrets(project), at(ProjectPage::Secrets)),
    (
      Link::Connection(project, id()),
      at(ProjectPage::Connections(ConnectionsView::default().with_selected(id()))),
    ),
    (
      Link::Account(id()),
      Route::Accounts(AccountsView::of(id(), AccountsPanel::Account)),
    ),
    (
      Link::Usage(project, UsageLink::default()),
      at(ProjectPage::Usage(UsageLink::default())),
    ),
    (
      Link::Usage(project, every_filter()),
      at(ProjectPage::Usage(every_filter())),
    ),
    (
      Link::PullRequest(project, number(42)),
      at(ProjectPage::PullRequest(number(42))),
    ),
  ]
}

#[test]
fn links_cover_every_kind_the_builder_makes() {
  let mut covered = [false; KINDS];
  for (link, _) in every_link() {
    covered[kind(&link)] = true;
  }
  assert!(covered.iter().all(|kind| *kind), "every kind has a case: {covered:?}");
}

#[test]
fn links_read_as_their_route_and_write_back_the_same() {
  for (link, route) in every_link() {
    let path = link.path();
    assert_eq!(Route::parse(&path), Ok(route.clone()), "{path}");
    assert_eq!(route.path(), path, "the route writes the link back");
  }
}

#[test]
fn links_resolve_to_their_screen_in_the_router_table() {
  let table = routes();
  for (link, route) in every_link() {
    let path = link.path();
    let chain = table.resolve(&path);
    let leaf = chain.last().unwrap_or_else(|| panic!("{path} resolves"));
    assert_eq!(leaf.pattern_raw(), route.pattern(), "{path}");
  }
}

#[test]
fn links_have_the_documented_paths() {
  let root = format!("/project/{PROJECT}");
  let mut usage = UsageLink::default();
  usage.set(UsageParam::From, "2026-10-01").expect("a day");
  usage.set(UsageParam::To, "2026-10-15").expect("a day");
  usage.set(UsageParam::Group, "task").expect("a grouping");
  usage.set(UsageParam::Agent, ID).expect("a profile id");
  for (link, path) in [
    (
      Link::Task(project(), key(), TaskPage::Conversation),
      format!("{root}/tasks/ORC-24/conversation"),
    ),
    (
      Link::Usage(project(), usage),
      format!("{root}/usage?from=2026-10-01&to=2026-10-15&group=task&agent={ID}"),
    ),
    (
      Link::PullRequest(project(), number(7)),
      format!("{root}/pull-requests/7"),
    ),
    (Link::Account(id()), format!("/settings?tab=accounts&account={ID}")),
  ] {
    assert_eq!(link.path(), path);
  }
}

#[test]
fn links_usage_groupings_read_back_and_bad_filters_say_which() {
  for group in UsageGroup::ALL {
    let mut filters = UsageLink::default();
    filters.set(UsageParam::Group, group.name()).expect("a grouping");
    let link = Link::Usage(project(), filters.clone());
    assert_eq!(Route::parse(&link.path()), Ok(at(ProjectPage::Usage(filters))));
  }
  for (query, param, value) in [
    ("from=2026-13-01", "from", "2026-13-01"),
    ("group=week", "group", "week"),
    ("run=7", "run", "7"),
  ] {
    let path = format!("/project/{PROJECT}/usage?{query}");
    assert_eq!(
      Route::parse(&path),
      Err(RouteError::InvalidParam {
        param: param.to_owned(),
        value: value.to_owned(),
      }),
      "{path}"
    );
  }
  for number in ["seven", "0"] {
    assert_eq!(
      Route::parse(&format!("/project/{PROJECT}/pull-requests/{number}")),
      Err(RouteError::NoScreen),
      "pull request {number}"
    );
  }
}

#[test]
fn links_written_without_a_tasks_tab_open_its_summary() {
  let path = format!("/project/{PROJECT}/tasks/ORC-24");
  let route = Route::parse(&path).expect("a task's page");
  assert_eq!(route, at(ProjectPage::Task(key(), TaskView::Summary)));
  assert_eq!(route.path(), Link::Task(project(), key(), TaskPage::Summary).path());
  let chain = routes().resolve(&path);
  let leaf = chain.last().unwrap_or_else(|| panic!("{path} resolves"));
  assert_eq!(
    leaf.pattern_raw(),
    "/project/:project/tasks/:key",
    "a route of its own, not the fallback"
  );
  assert_eq!(Route::parse(&format!("{path}/summary/more")), Err(RouteError::NoScreen));
}
