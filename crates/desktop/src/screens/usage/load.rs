//! What the Usage dashboard reads, off the UI thread: the usage of the project shown, or of every
//! listed project, and the plan limits history of every listed project, since plan limits belong to
//! the CLI accounts and every project's runs report them (decided 2026-09-30).

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use lurq::app::theme::SpacingSize;
use orchester_core::{
  id::ProjectId,
  time::Timestamp,
  usage::{
    ProjectFacts,
    limits::{LimitSnapshot, WarningThreshold, retained_since},
  },
};
use orchester_store::{ProjectDb, ReadOnlyProjectDb, StoreError};

use crate::{
  messages::{self, Message},
  theme::ExtraSpacing,
  workspace::{OpenProject, Workspace},
};

/// Whose usage the dashboard shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
  Project(OpenProject),
  /// Every listed project (All projects › Usage).
  All,
}

impl lurq::app::component::DevtoolsInspectable for Scope {}

impl Scope {
  /// The dashboard's side padding: the project page's 32, or All projects' page padding, which
  /// leaves the sections 1344 wide in a 1440 window (`p4eVV6`).
  pub fn inset(&self) -> SpacingSize {
    match self {
      Self::Project(_) => SpacingSize::Section,
      Self::All => ExtraSpacing::Page.role(),
    }
  }
}

/// What the dashboard read.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageData {
  pub projects: Arc<[ProjectFacts]>,
  /// Where each project of `projects` is, to open its tasks and runs.
  pub roots: Arc<HashMap<ProjectId, PathBuf>>,
  /// Listed projects whose usage could not be read, by name.
  pub unread: Arc<[Arc<str>]>,
  /// Every listed project's plan limits reports of the last 90 days.
  pub snapshots: Arc<[LimitSnapshot]>,
  pub threshold: WarningThreshold,
  pub read_at: Timestamp,
}

impl lurq::app::component::DevtoolsInspectable for UsageData {}

/// Reads the usage of `scope` from `since` on.
pub fn read(workspace: &Workspace, scope: &Scope, since: Timestamp) -> Result<UsageData, Message> {
  let now = Timestamp::now();
  let failed = |error: StoreError| {
    tracing::error!(%error, "cannot read the usage of the project shown");
    messages::store_error(&error)
  };
  let mut data = UsageData {
    projects: Arc::from([]),
    roots: Arc::default(),
    unread: Arc::from([]),
    snapshots: Arc::from([]),
    threshold: workspace.usage_warning_threshold().map_err(failed)?,
    read_at: now,
  };
  let (mut projects, mut roots, mut unread, mut snapshots) = (Vec::new(), HashMap::new(), Vec::new(), Vec::new());
  if let Scope::Project(open) = scope {
    let db = ProjectDb::open_read_only(&open.repo_root).map_err(failed)?;
    projects.push(facts(&db, since, now).map_err(failed)?);
    roots.insert(open.project.id(), open.repo_root.clone());
  }
  for registered in workspace.projects().map_err(failed)? {
    let db = match ProjectDb::open_read_only(&registered.repo_root) {
      Ok(db) => db,
      Err(error) => {
        // Its share is missing: All projects names it, and its plan limits reports are not drawn.
        tracing::warn!(%error, repo_root = %registered.repo_root.display(), "cannot read a listed project's usage");
        unread.push(Arc::from(registered.name.as_str()));
        continue;
      }
    };
    if *scope == Scope::All {
      match facts(&db, since, now) {
        Ok(read) => {
          projects.push(read);
          roots.insert(registered.id, registered.repo_root.clone());
        }
        Err(error) => {
          tracing::error!(%error, repo_root = %registered.repo_root.display(), "cannot read a listed project's usage");
          unread.push(Arc::from(registered.name.as_str()));
        }
      }
    }
    match db.limit_snapshots(retained_since(now)) {
      Ok(read) => snapshots.extend(read),
      Err(error) => {
        tracing::error!(%error, repo_root = %registered.repo_root.display(), "cannot read a project's plan limits history");
      }
    }
  }
  if matches!(scope, Scope::Project(_)) {
    // Only the project shown is the dashboard's; others matter here for their plan limits.
    unread.clear();
  }
  data.projects = projects.into();
  data.roots = Arc::new(roots);
  data.unread = unread.into();
  data.snapshots = snapshots.into();
  Ok(data)
}

fn facts(db: &ReadOnlyProjectDb, since: Timestamp, now: Timestamp) -> Result<ProjectFacts, StoreError> {
  Ok(ProjectFacts {
    project: db.project().id(),
    name: db.project().name().as_str().into(),
    facts: db.usage_facts(since, now)?,
  })
}

/// Saves the warning threshold for every project.
pub fn save_threshold(workspace: &Workspace, threshold: WarningThreshold) -> Result<WarningThreshold, Message> {
  workspace.set_usage_warning_threshold(threshold).map_err(|error| {
    tracing::error!(%error, "cannot save the usage warning threshold");
    messages::store_error(&error)
  })?;
  Ok(threshold)
}
