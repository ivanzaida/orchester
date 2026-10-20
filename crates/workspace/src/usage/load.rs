//! What the Usage page and `get_usage` read: the usage facts of a project, or of every listed project,
//! from a time on, and the plan limits history of every listed project, since plan limits belong to the
//! CLI accounts and every project's runs report them (decided 2026-09-30), with what Orchester read of
//! them outside runs. Blocks on SQLite.

use std::{path::Path, sync::Arc};

use orchester_core::{
  account::AgentAccount,
  time::Timestamp,
  usage::{
    ProjectFacts,
    limits::{LimitSnapshot, WarningThreshold, retained_since},
  },
};
use orchester_store::{ProjectDb, ReadOnlyProjectDb, StoreError};

use crate::workspace::Workspace;

/// What was read.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageData {
  pub projects: Arc<[ProjectFacts]>,
  /// Listed projects whose usage could not be read, by name; their share is missing.
  pub unread: Arc<[Arc<str>]>,
  /// How many runs of the projects read have a damaged record; their usage is left out, the rest counted.
  pub unread_runs: usize,
  /// Every listed project's plan limits reports of the last 90 days.
  pub snapshots: Arc<[LimitSnapshot]>,
  pub threshold: WarningThreshold,
  /// The CLI accounts, which name the accounts a filter chooses.
  pub accounts: Arc<[AgentAccount]>,
  pub read_at: Timestamp,
}

/// Whose usage is read.
#[derive(Clone, Copy, Debug)]
pub enum Scope<'a> {
  /// The project at this folder.
  Project(&'a Path),
  /// Every listed project.
  All,
}

impl UsageData {
  /// The usage of `scope` from `since` on, as it stands at `now`.
  pub fn read(workspace: &Workspace, scope: Scope<'_>, since: Timestamp, now: Timestamp) -> Result<Self, StoreError> {
    let mut projects = Vec::new();
    let (mut unread, mut snapshots, mut unread_runs) = (Vec::new(), Vec::new(), 0);
    if let Scope::Project(root) = scope {
      projects.push(facts(&ProjectDb::open_read_only(root)?, since, now, &mut unread_runs)?);
    }
    for registered in workspace.projects()? {
      let db = match ProjectDb::open_read_only(&registered.repo_root) {
        Ok(db) => db,
        Err(error) => {
          // Its share is missing: the answer names it, and its plan limits reports are not drawn.
          tracing::warn!(%error, repo_root = %registered.repo_root.display(), "cannot read a listed project's usage");
          unread.push(Arc::from(registered.name.as_str()));
          continue;
        }
      };
      if matches!(scope, Scope::All) {
        match facts(&db, since, now, &mut unread_runs) {
          Ok(read) => projects.push(read),
          Err(error) => {
            tracing::error!(%error, repo_root = %registered.repo_root.display(), "cannot read a listed project's usage");
            unread.push(Arc::from(registered.name.as_str()));
          }
        }
      }
      match db.limit_snapshots(retained_since(now)) {
        Ok(read) => snapshots.extend(read),
        // Plan limits are a part of the page; the rest still reads.
        Err(error) => {
          tracing::error!(%error, repo_root = %registered.repo_root.display(), "cannot read a project's plan limits history");
        }
      }
    }
    if matches!(scope, Scope::Project(_)) {
      // Only the project shown is read for its usage; others matter here for their plan limits.
      unread.clear();
    }
    // What Orchester read outside runs belongs to the accounts, not to a project.
    match workspace.plan_reads(retained_since(now)) {
      Ok(read) => snapshots.extend(read),
      Err(error) => tracing::error!(%error, "cannot read the plan limits read outside runs"),
    }
    Ok(Self {
      projects: projects.into(),
      unread: unread.into(),
      unread_runs,
      snapshots: snapshots.into(),
      threshold: workspace.usage_warning_threshold()?,
      accounts: workspace.agent_accounts()?.into(),
      read_at: now,
    })
  }
}

/// The project's usage facts from `since` on, adding to `unread_runs` its runs whose record is damaged, which are
/// logged by id, never by what they hold.
fn facts(
  db: &ReadOnlyProjectDb,
  since: Timestamp,
  now: Timestamp,
  unread_runs: &mut usize,
) -> Result<ProjectFacts, StoreError> {
  for run in db.unreadable_runs_since(since)? {
    tracing::warn!(%run, project = %db.project().id(), "a run's record is damaged; its usage is left out");
    *unread_runs += 1;
  }
  Ok(ProjectFacts {
    project: db.project().id(),
    name: db.project().name().as_str().into(),
    facts: db.usage_facts(since, now)?,
  })
}
