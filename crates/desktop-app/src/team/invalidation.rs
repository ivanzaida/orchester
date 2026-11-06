//! What a team project's subscription changes in the session's cached reads: an invalidated scope makes the queries
//! that read it stale, and mounted views read them again; a reset or a reconnect makes every one of them stale. Bursts
//! coalesce twice: the reads' [`Changes`] gather every invalidation between two looks, and the pump looks at most once
//! per [`COALESCE`].

use std::time::Duration;

use lurq::query::QueryClient;
use orchester_api::v2::QueryScope;
use orchester_workspace::team_reads::{Changes, TeamProject, TeamReadError};

use super::{
  pages::{team_module, team_module_list, team_task},
  queries::{team_admins, team_capabilities, team_counts, team_modules, team_policy, team_tasks},
};

/// The shortest time between two rounds of invalidations of one project.
pub const COALESCE: Duration = Duration::from_millis(150);

/// Which of the session's queries read a scope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stale {
  pub tasks: bool,
  pub workspace: bool,
  pub modules: bool,
}

impl Stale {
  /// Every query: after a reset or a reconnect, or when the person asks to read again.
  pub const EVERY: Self = Self {
    tasks: true,
    workspace: true,
    modules: true,
  };

  /// What `changes` make stale.
  pub fn of(changes: &Changes) -> Self {
    if changes.is_reset() {
      return Self::EVERY;
    }
    let mut stale = Self::default();
    for scope in changes.scopes().keys() {
      match scope {
        QueryScope::TaskList {} | QueryScope::Task { .. } => stale.tasks = true,
        QueryScope::Workspace {} => stale.workspace = true,
        // A module's progress counts its tasks; a task's move changes both.
        QueryScope::ModuleList {} | QueryScope::Module { .. } => stale.modules = true,
        // The desktop reads no release from the server yet.
        QueryScope::TaskComments { .. }
        | QueryScope::Library {}
        | QueryScope::Resource { .. }
        | QueryScope::ReleaseList {}
        | QueryScope::Release { .. } => {}
      }
    }
    stale
  }

  /// Makes those queries stale in `client`.
  pub fn apply(self, client: &QueryClient) {
    if self.tasks {
      client.invalidate(team_tasks::all());
      client.invalidate(team_counts::all());
      client.invalidate(team_task::all());
    }
    if self.workspace {
      client.invalidate(team_capabilities::all());
      client.invalidate(team_admins::all());
      client.invalidate(team_policy::all());
    }
    if self.modules {
      client.invalidate(team_modules::all());
      client.invalidate(team_module_list::all());
      client.invalidate(team_module::all());
    }
  }
}

/// Applies `project`'s changes to `client` until its reads end; returns how they ended.
pub async fn pump(project: TeamProject, client: QueryClient) -> TeamReadError {
  loop {
    match project.changes().await {
      Ok(changes) => Stale::of(&changes).apply(&client),
      Err(ended) => return ended,
    }
    tokio::time::sleep(COALESCE).await;
  }
}
