//! Links into the app (`docs/design/orchestrator-toolset.md`, "Links"): the route path of a place in
//! `orchester-app`'s window, the same string its router (`crates/desktop-app/src/routes`) parses. Every `link`
//! a domain tool returns is made here, and the router's tests parse every kind made here, so the two do not
//! drift. A path names a project by its id, which never changes, and a task by its key, as people read it.
//! A place whose page is not built yet still has its link; the router shows the page's placeholder.

mod query;

use std::num::NonZeroU64;

use orchester_core::{
  id::{AgentAccountId, ConnectionId, ProjectId, ResourceId},
  project::TaskKey,
};

pub use self::query::{encode, with_query};
use crate::pull_requests::list::PullRequestTab;

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
}

/// The tab of a task's page a link opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskPage {
  /// What the task is and where it stands, with its runs.
  Summary,
  /// The agents' conversation, with the cards of what waits for a person: approvals, questions, plans.
  Conversation,
  /// What the task's latest run changed.
  Changes,
}

impl TaskPage {
  /// The tab's segment, as the router names it.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Summary => "summary",
      Self::Conversation => "conversation",
      Self::Changes => "changes",
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
    }
  }
}

fn root(project: ProjectId) -> String {
  format!("/project/{project}")
}
