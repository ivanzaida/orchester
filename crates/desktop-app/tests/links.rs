//! Every kind of link `orchester_workspace::links` makes, the links the domain tools return
//! (`docs/design/orchestrator-toolset.md`, "Links"), reads as the route of its place, writes back as the same
//! path and resolves in the router's table to that route's screen, so a link a tool returns opens its place.
//! A page that is not built yet has its route too. A Usage link carries its whole query.

use std::num::NonZeroU64;

use chrono::NaiveDate;
use lurq::router::RouteMatch;
use orchester_app::routes::{
  AccountsPanel, AccountsView, ConnectionsView, FilesView, OrchestratorChat, OverviewTab, ProjectPage, Route,
  RouteError, TaskView, ToolTab, routes,
};
use orchester_core::{
  connection::AgentProvider,
  id::{AgentAccountId, ProjectId, RunId},
  project::TaskKey,
  revision::Revision,
  usage::{DayRange, GroupBy, Period},
};
use orchester_workspace::{
  delivery::DeliveryPeriod,
  files::{FilePath, LineRange, Version},
  links::{Link, TaskPage},
  pull_requests::list::PullRequestTab,
  usage::UsageQuery,
};

const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";
const ID: &str = "7e1f2a3b-4c5d-4e6f-8a9b-0c1d2e3f4a5b";

/// How many kinds of link there are: one per variant of [`Link`].
const KINDS: usize = 19;

/// The pattern the table matched for a path, whole: the layouts' and the route's under the shell's.
fn matched(chain: &[RouteMatch]) -> String {
  chain.iter().skip(1).map(RouteMatch::pattern_raw).collect()
}

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
    Link::File { .. } => 11,
    Link::PullRequests(..) => 12,
    Link::Usage(..) => 13,
    Link::UsageEverywhere(_) => 14,
    Link::Modules(_) => 15,
    Link::Module(..) => 16,
    Link::Orchestrator(..) => 17,
    Link::Delivery(..) => 18,
  }
}

/// A query with every parameter a Usage link carries, repeated values among them, and one with `period`.
fn usage_queries() -> [UsageQuery; 2] {
  let day = |day: u32| NaiveDate::from_ymd_opt(2026, 10, day).expect("an October day");
  let mut every = UsageQuery {
    period: Period::Custom(DayRange::new(day(1), day(15)).expect("a range")),
    group: GroupBy::Step,
    ..UsageQuery::default()
  };
  let filter = &mut every.filter;
  filter
    .clis
    .extend([AgentProvider::CodexCli, AgentProvider::ClaudeCodeCli]);
  filter.models.extend(["gpt-5".into(), "claude opus & co".into()]);
  filter.profiles.push(ID.into());
  filter.pipelines.push("Quick fix #2".into());
  filter.tags.extend(["auth".into(), "billing".into()]);
  filter.tasks.push("ORC-24".into());
  filter.runs.push(RunId::generate());
  filter.steps.push("Implement".into());
  filter.accounts.push(AgentAccountId::generate());
  filter.no_account = true;
  let today = UsageQuery {
    period: Period::Today,
    group: GroupBy::Hour,
    ..UsageQuery::default()
  };
  [every, today]
}

/// Each tab of a task, with the route its link names.
fn task_links(project: ProjectId) -> Vec<(Link, Route)> {
  [
    (TaskPage::Details, TaskView::Details),
    (TaskPage::Summary, TaskView::Summary),
    (TaskPage::Conversation, TaskView::Conversation),
    (TaskPage::Changes, TaskView::Changes(None)),
    (TaskPage::LocalIntegration, TaskView::LocalIntegration),
  ]
  .into_iter()
  .map(|(page, view)| (Link::Task(project, key(), page), at(ProjectPage::Task(key(), view))))
  .collect()
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
      Link::Pipeline(project, id()),
      at(ProjectPage::Pipeline(id(), Default::default())),
    ),
    (
      Link::Profile(project, id()),
      at(ProjectPage::Agent(id(), Default::default())),
    ),
    (
      Link::Instructions(project),
      at(ProjectPage::Instructions(Default::default())),
    ),
    (Link::Skill(project, id()), at(ProjectPage::Skill(id()))),
    (
      Link::Tool(project, id()),
      at(ProjectPage::Tool(id(), ToolTab::Configuration)),
    ),
    (Link::Secrets(project), at(ProjectPage::Secrets(Default::default()))),
    (
      Link::Connection(project, id()),
      at(ProjectPage::Connections(ConnectionsView::of(id()))),
    ),
    (
      Link::Account(id()),
      Route::Accounts(AccountsView::of(id(), AccountsPanel::Account)),
    ),
    (
      Link::PullRequest(project, number(42)),
      at(ProjectPage::PullRequest(number(42), None)),
    ),
    (
      Link::PullRequests(project, PullRequestTab::Open),
      at(ProjectPage::PullRequests(PullRequestTab::Open)),
    ),
    (
      Link::PullRequests(project, PullRequestTab::NeedsReview),
      at(ProjectPage::PullRequests(PullRequestTab::NeedsReview)),
    ),
    (Link::Modules(project), at(ProjectPage::Modules)),
    (Link::Module(project, id()), at(ProjectPage::Module(id()))),
    (
      Link::Orchestrator(project, id()),
      at(ProjectPage::Orchestrator(OrchestratorChat::Chat(id()))),
    ),
  ]
  .into_iter()
  .chain(task_links(project))
  .chain(file_links(project))
  .chain(DeliveryPeriod::ALL.map(|period| (Link::Delivery(project, period), at(ProjectPage::Delivery(period)))))
  .chain(usage_queries().into_iter().flat_map(|query| {
    let mut everywhere = query.clone();
    everywhere.filter.projects.extend([project, ProjectId::generate()]);
    [
      (
        Link::Usage(project, Box::new(query.clone())),
        at(ProjectPage::Usage(Box::new(query))),
      ),
      (
        Link::UsageEverywhere(Box::new(everywhere.clone())),
        Route::Usage(Box::new(everywhere)),
      ),
    ]
  }))
  .collect()
}

/// A file at every version, with and without lines, its path with characters a route escapes and with `/`s
/// that would leave empty segments.
fn file_links(project: ProjectId) -> Vec<(Link, Route)> {
  let commit = Revision::new("4d2b8e0c1f2a3b4c5d6e7f8091a2b3c4d5e6f708").expect("a commit");
  let mut links = Vec::new();
  for at in [
    Version::Checkout,
    Version::Base,
    Version::Run(id()),
    Version::Revision(commit),
  ] {
    for path in [
      "crates/workspace/src/plan_limits.rs",
      "docs/a b & c#1.md",
      "notes/слово.md",
    ] {
      for line in [None, LineRange::new(441, 442)] {
        let file = FilePath::new(path).expect("a project path");
        let link = Link::File {
          project,
          path: file,
          at: at.clone(),
          line,
        };
        let view = FilesView::new(Some(path), at.clone()).with_line(line);
        links.push((link, at_files(view)));
      }
    }
  }
  links
}

fn at_files(view: FilesView) -> Route {
  at(ProjectPage::Files(view))
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
    assert!(!chain.is_empty(), "{path} resolves");
    assert_eq!(matched(&chain), route.pattern(), "{path}");
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
    (
      Link::File {
        project: project(),
        path: FilePath::new("src/a b.rs").expect("a project path"),
        at: Version::Base,
        line: LineRange::new(12, 20),
      },
      format!("{root}/files/src/a%20b.rs?at=base&line=12-20"),
    ),
    (
      Link::Usage(project(), Box::new(usage_queries()[1].clone())),
      format!("{root}/usage?period=today"),
    ),
    (
      Link::UsageEverywhere(Box::new(UsageQuery {
        group: GroupBy::Model,
        ..UsageQuery::default()
      })),
      "/usage?group=model".to_owned(),
    ),
    (
      Link::Delivery(project(), DeliveryPeriod::Month),
      format!("{root}/usage/delivery"),
    ),
    (
      Link::Delivery(project(), DeliveryPeriod::Quarter),
      format!("{root}/usage/delivery?period=90d"),
    ),
  ] {
    assert_eq!(link.path(), path);
  }
}

#[test]
fn links_carry_an_account_and_no_account_and_read_back() {
  let mut query = UsageQuery::default();
  query.filter.accounts.push(id());
  query.filter.no_account = true;
  let link = Link::UsageEverywhere(Box::new(query.clone()));
  let path = link.path();
  assert_eq!(path, format!("/usage?account={ID}&account=none"));
  assert_eq!(Route::parse(&path), Ok(Route::Usage(Box::new(query))));
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
fn links_written_without_a_tasks_tab_open_its_details() {
  let path = format!("/project/{PROJECT}/tasks/ORC-24");
  let route = Route::parse(&path).expect("a task's page");
  assert_eq!(route, at(ProjectPage::Task(key(), TaskView::Details)));
  assert_eq!(route.path(), Link::Task(project(), key(), TaskPage::Details).path());
  let chain = routes().resolve(&path);
  let leaf = chain.last().unwrap_or_else(|| panic!("{path} resolves"));
  assert_eq!(
    leaf.pattern_raw(),
    "/project/:project/tasks/:key",
    "a route of its own, not the fallback"
  );
  assert_eq!(Route::parse(&format!("{path}/summary/more")), Err(RouteError::NoScreen));
}

#[test]
fn links_naming_a_tasks_summary_still_open_it() {
  let path = format!("/project/{PROJECT}/tasks/ORC-24/summary");
  assert_eq!(Route::parse(&path), Ok(at(ProjectPage::Task(key(), TaskView::Summary))));
  assert_eq!(path, Link::Task(project(), key(), TaskPage::Summary).path());
}
