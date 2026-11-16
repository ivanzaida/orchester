//! The pipelines a project's tasks can use, wherever they live: a local project's in its database, a team project's in
//! the team's configuration as this session read it from the team server (`docs/design/team-configuration-live.md`),
//! never an older copy.

use std::{path::Path, sync::Arc};

use orchester_core::resource::{Pipeline, ResourceKind};
use orchester_store::{ProjectDb, StoreError};

use super::data::{PipelineChoice, ProjectTasks, choice, published_pipelines};
use crate::{
  pipelines::data::Assignables,
  team_config::{TeamConfig, TeamNotRead, TeamRead},
  team_reads::listed::TeamPipelines,
  workspace::Workspace,
};

/// Why a project's pipelines could not be read.
#[derive(Debug, thiserror::Error)]
pub enum PipelinesUnread {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// A team project's configuration is being read, or could not be read this session.
  #[error(transparent)]
  Team(#[from] TeamNotRead),
}

/// The team's pipelines in `config` with a published version a task can use, by name; `db` gives what this computer
/// filled in for their profiles. One whose latest version is not in the team's form is left out: it cannot be run here.
pub fn team_published_pipelines(db: &ProjectDb, config: &TeamConfig) -> Result<Vec<PipelineChoice>, StoreError> {
  let assignables = Assignables::of_team(db, config)?;
  Ok(
    config
      .resources(ResourceKind::Pipeline)
      .into_iter()
      .filter_map(|resource| match (resource.number(), resource.body::<Pipeline>()) {
        (Some(number), Some(Ok(body))) => Some(choice(
          (resource.id, resource.name.as_str()),
          number,
          &body,
          &assignables,
        )),
        _ => None,
      })
      .collect(),
  )
}

impl Workspace {
  /// The pipelines the tasks of the project at `repo_root`, whose database is `db`, can use: a team project's from the
  /// team's configuration read this session, a local project's from `db`. Blocks on SQLite.
  pub fn project_pipelines(&self, db: &ProjectDb, repo_root: &Path) -> Result<Vec<PipelineChoice>, PipelinesUnread> {
    if db.sharing()?.is_none() {
      return Ok(published_pipelines(db)?);
    }
    let config = self.team_read(repo_root).loaded()?;
    Ok(team_published_pipelines(db, &config)?)
  }
}

impl ProjectTasks {
  /// A team project's tasks with the team's pipelines as this session read them (`read`), in place of the none
  /// [`ProjectTasks::load`] gives it; a local project's as they are. While the configuration is being read, or could
  /// not be read, it offers none, which it logs: an older copy is never offered.
  pub fn with_team_pipelines(mut self, repo_root: &Path, read: TeamRead) -> Result<Self, StoreError> {
    if !self.shared {
      return Ok(self);
    }
    match read.loaded() {
      Ok(config) => {
        let db = ProjectDb::open(repo_root)?;
        self.pipelines = Arc::new(team_published_pipelines(&db, &config)?);
        let team = TeamPipelines::of(&config);
        self.missing_pipelines = Arc::new(
          self
            .tasks
            .iter()
            .filter_map(|task| task.details().pipeline.map(|pipeline| (task.id(), pipeline)))
            .filter(|(_, pipeline)| !team.contains(*pipeline))
            .map(|(task, pipeline)| (task, pipeline.to_string()))
            .collect(),
        );
      }
      Err(unread) => {
        tracing::info!(%unread, "a team project's pipelines are not offered until its configuration is read")
      }
    }
    Ok(self)
  }
}

impl PipelinesUnread {
  /// What a person is told.
  pub fn message(&self) -> crate::messages::Message {
    match self {
      Self::Store(error) => crate::messages::store_error(error),
      Self::Team(unread) => unread.message(),
    }
  }
}
