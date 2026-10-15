//! Every kind of link `orchester_workspace::links` makes, the links the domain tools return
//! (`docs/design/orchestrator-toolset.md`, "Links"), reads as the route of its place, writes back as the same
//! path and resolves in the router's table to that route's screen, so a link a tool returns opens its place.
//! A page that is not built yet has its route too.

use std::num::NonZeroU64;

use orchester_app::routes::{
  AccountsPanel, AccountsView, ConnectionsView, OverviewTab, ProjectPage, Route, RouteError, TaskView, ToolTab, routes,
};
use orchester_core::{id::ProjectId, project::TaskKey};
use orchester_workspace::{
  links::{Link, TaskPage},
  pull_requests::list::PullRequestTab,
};

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
    Link::PullRequest(..) => 10,
    Link::PullRequests(..) => 11,
  }
}

/// Every kind of link and each tab of a task, with the route each names.
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
      Link::PullRequest(project, number(42)),
      at(ProjectPage::PullRequest(number(42))),
    ),
    (
      Link::PullRequests(project, PullRequestTab::Open),
      at(ProjectPage::PullRequests(PullRequestTab::Open)),
    ),
    (
      Link::PullRequests(project, PullRequestTab::NeedsReview),
      at(ProjectPage::PullRequests(PullRequestTab::NeedsReview)),
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
  for (link, path) in [
    (
      Link::Task(project(), key(), TaskPage::Conversation),
      format!("{root}/tasks/ORC-24/conversation"),
    ),
    (
      Link::PullRequest(project(), number(7)),
      format!("{root}/pull-requests/7"),
    ),
    (
      Link::PullRequests(project(), PullRequestTab::Open),
      format!("{root}/pull-requests"),
    ),
    (
      Link::PullRequests(project(), PullRequestTab::Merged),
      format!("{root}/pull-requests?tab=merged"),
    ),
    (Link::Account(id()), format!("/settings?tab=accounts&account={ID}")),
  ] {
    assert_eq!(link.path(), path);
  }
}

#[test]
fn links_pull_requests_are_numbered_from_one() {
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
