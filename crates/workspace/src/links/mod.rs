//! Links into the app (`docs/design/orchestrator-toolset.md`, "Links"): the route path of a place in
//! `orchester-app`'s window, the same string its router (`crates/desktop-app/src/routes`) parses. Every `link`
//! a domain tool returns is made here, and the router's tests parse every kind made here, so the two do not
//! drift. A path names a project by its id, which never changes, and a task by its key, as people read it.
//! A place whose page is not built yet still has its link; the router shows the page's placeholder.

mod query;
mod usage;

use std::num::NonZeroU64;

use orchester_core::{
  id::{AgentAccountId, ChatId, ConnectionId, ModuleId, ProjectId, ResourceId},
  project::TaskKey,
};

pub use self::{
  query::{encode, encode_path, with_query},
  usage::{SEGMENT as USAGE, with_usage_query},
};

/// The segment of a project's Modules list, and of a module's page under it.
pub const MODULES: &str = "modules";
use crate::{
  files::{FilePath, LineRange, Version},
  pull_requests::list::PullRequestTab,
  usage::UsageQuery,
};

/// A place in the app a tool result points to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
  /// A project's Overview.
  Project(ProjectId),
  /// A task's page on one of its tabs.
  Task(ProjectId, TaskKey, TaskPage),
  /// A pipeline's editor.
  Pipeline(ProjectId, ResourceId),
  /// An agent profile in Agents.
  Profile(ProjectId, ResourceId),
  /// Instructions, which open on their library: no route names one instruction yet.
  Instructions(ProjectId),
  /// A skill in Skills.
  Skill(ProjectId, ResourceId),
  /// A tool's editor on its Configuration tab.
  Tool(ProjectId, ResourceId),
  /// Secrets, which open on their list: no route names one secret yet.
  Secrets(ProjectId),
  /// Connections with the inspector of an agent or GitHub connection, or of an MCP server.
  Connection(ProjectId, ConnectionId),
  /// Settings › Accounts showing an account; accounts belong to the person, not to a project.
  Account(AgentAccountId),
  /// The Pull requests page on one of its tabs.
  PullRequests(ProjectId, PullRequestTab),
  /// A pull request of the project's repository, by its number.
  PullRequest(ProjectId, NonZeroU64),
  /// A file of the project in the code viewer, at a version, on a range of its lines.
  File {
    project: ProjectId,
    path: FilePath,
    at: Version,
    line: Option<LineRange>,
  },
  /// A project's Usage page showing a query (boxed: a query holds many filters).
  Usage(ProjectId, Box<UsageQuery>),
  /// Every project's usage, All projects' *Usage* tab, showing a query.
  UsageEverywhere(Box<UsageQuery>),
  /// A project's Modules list.
  Modules(ProjectId),
  /// A module's page, with its tasks and its burndown.
  Module(ProjectId, ModuleId),
  /// A conversation of the project's Orchestrator chat.
  Orchestrator(ProjectId, ChatId),
}

/// The tab of a task's page a link opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskPage {
  /// The task itself, its description and acceptance criteria: the tab a task opens on, for a link to the task.
  Details,
  /// The agents' conversation, with the cards of what waits for a person: approvals, questions, plans.
  Conversation,
  /// What the task's latest run changed.
  Changes,
  /// Where the task stands, with its runs: for a link to a run or a decision on one.
  Summary,
  /// Merging the latest run's work beside the person's uncommitted work, where a merge that stopped carries on.
  LocalIntegration,
}

impl TaskPage {
  /// The tab's segment, as the router names it.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Details => "details",
      Self::Conversation => "conversation",
      Self::Changes => "changes",
      Self::Summary => "summary",
      Self::LocalIntegration => "local-integration",
    }
  }
}

impl Link {
  /// The link as the app's router reads it, query included.
  pub fn path(&self) -> String {
    match self {
      Self::Project(project) => root(*project),
      Self::Task(project, key, page) => format!("{}/tasks/{key}/{}", root(*project), page.name()),
      Self::Pipeline(project, id) => format!("{}/pipelines/{id}", root(*project)),
      Self::Profile(project, id) => format!("{}/agents/{id}", root(*project)),
      Self::Instructions(project) => format!("{}/instructions", root(*project)),
      Self::Skill(project, id) => with_query(
        format!("{}/skills", root(*project)),
        &[("skill", Some(&id.to_string()))],
      ),
      Self::Tool(project, id) => with_query(
        format!("{}/tools", root(*project)),
        &[("tool", Some(&id.to_string())), ("tab", Some("configuration"))],
      ),
      Self::Secrets(project) => format!("{}/secrets", root(*project)),
      Self::Connection(project, id) => with_query(
        format!("{}/connections", root(*project)),
        &[("selected", Some(&id.to_string()))],
      ),
      Self::Account(id) => with_query(
        "/settings".to_owned(),
        &[("tab", Some("accounts")), ("account", Some(&id.to_string()))],
      ),
      Self::PullRequests(project, tab) => with_query(
        format!("{}/pull-requests", root(*project)),
        &[("tab", (*tab != PullRequestTab::default()).then(|| tab.name()))],
      ),
      Self::PullRequest(project, number) => format!("{}/pull-requests/{number}", root(*project)),
      Self::File {
        project,
        path,
        at,
        line,
      } => files_path(format!("{}/files", root(*project)), Some(path.as_str()), at, *line),
      Self::Usage(project, query) => with_usage_query(format!("{}/{USAGE}", root(*project)), query),
      Self::UsageEverywhere(query) => with_usage_query(format!("/{USAGE}"), query),
      Self::Modules(project) => format!("{}/{MODULES}", root(*project)),
      Self::Module(project, id) => format!("{}/{MODULES}/{id}", root(*project)),
      Self::Orchestrator(project, chat) => with_query(
        format!("{}/orchestrator", root(*project)),
        &[("chat", Some(&chat.to_string()))],
      ),
    }
  }
}

/// The code viewer's path under `files` (a project's `/project/<id>/files`): the file's path, if any, as
/// segments ([`encode_path`]), then `at` the version unless it is the checkout, and `line` the lines. The
/// router's Files route writes its paths with it too, so the two cannot differ.
pub fn files_path(files: String, file: Option<&str>, at: &Version, line: Option<LineRange>) -> String {
  let path = match file {
    Some(file) => format!("{files}/{}", encode_path(file)),
    None => files,
  };
  let at = (*at != Version::Checkout).then(|| at.to_string());
  let line = line.map(|line| line.to_string());
  with_query(path, &[("at", at.as_deref()), ("line", line.as_deref())])
}

fn root(project: ProjectId) -> String {
  format!("/project/{project}")
}
