//! The app routes of the Pull requests page, its pull requests and their tasks, as `link`s
//! (`docs/design/orchester-toolset.md`, "Links"): the same paths `crates/desktop-app/src/routes`
//! parses. They move to the shared `orchester_workspace::links` once it exists.

use orchester_core::{id::ProjectId, project::TaskKey};

use super::list::PullRequestTab;

/// The Pull requests page on `tab`.
pub fn pull_requests(project: ProjectId, tab: PullRequestTab) -> String {
  let page = format!("/project/{project}/pull-requests");
  match tab {
    PullRequestTab::Open => page,
    tab => format!("{page}?tab={}", tab.name()),
  }
}

/// Pull request `number` of the project's repository.
pub fn pull_request(project: ProjectId, number: u64) -> String {
  format!("/project/{project}/pull-requests/{number}")
}

/// The Summary of task `key`.
pub fn task(project: ProjectId, key: &TaskKey) -> String {
  format!("/project/{project}/tasks/{key}/summary")
}
