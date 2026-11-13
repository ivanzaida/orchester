//! A team project's pipelines as the Pipelines page shows them: the library and an opened pipeline from the team's
//! configuration as this session read it, and a pipeline's versions from the team server
//! (`docs/design/team-configuration-live.md`). An admin's save goes to the team server ([`super::team_write`]). Nothing
//! of the team's is read from the project database; a local project's pipelines are read from it as before.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind, VersionNumber},
  store::TaskStore,
  task::Task,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  data::{Assignables, OpenError, OpenedPipeline},
  history::VersionHistory,
  levels::levels,
  library::{LibraryEntry, library},
};
use crate::{
  background,
  messages::{self, Message},
  sharing::{Servers, SharingError, project::ProjectServer, sync::ConfigServer},
  tasks::team_pipelines::PipelinesUnread,
  team_config::{TeamConfig, TeamResource},
  workspace::Workspace,
};

impl Workspace {
  /// The pipelines the library of the project at `repo_root` lists: a team project's from the team's configuration
  /// read this session, a local project's from its database. Blocks on SQLite.
  pub fn pipeline_library(&self, repo_root: &Path) -> Result<Vec<LibraryEntry>, PipelinesUnread> {
    let db = ProjectDb::open(repo_root)?;
    if db.sharing()?.is_none() {
      return Ok(library(&db)?);
    }
    let config = self.team_read(repo_root).loaded()?;
    Ok(team_library(&config, &db.tasks()?))
  }

  /// Pipeline `id` of the project at `repo_root` as the editor opens it: a team project's latest version from the
  /// team's configuration read this session, a local project's from its database. Blocks.
  pub fn open_pipeline(&self, repo_root: &Path, id: ResourceId) -> Result<OpenedPipeline, OpenError> {
    let db = ProjectDb::open(repo_root)?;
    if db.sharing()?.is_none() {
      return OpenedPipeline::load(&db, id);
    }
    let config = self.team_read(repo_root).loaded()?;
    let resource = config.live(id).ok_or(OpenError::Missing)?;
    let (version, published_at, body) = published(resource).ok_or(OpenError::Missing)?;
    Ok(OpenedPipeline {
      id,
      name: resource.name.as_str().into(),
      slug: resource.slug.as_str().into(),
      body,
      version,
      published_at,
      used_by: db
        .tasks()?
        .iter()
        .filter(|task| task.details().pipeline == Some(id))
        .count(),
      assignables: Arc::new(Assignables::of_team(&db, &config)?),
      imported: None,
      team: true,
    })
  }
}

/// The team's pipelines in `config` with a published version, by name; `tasks` say how many tasks use each.
pub fn team_library(config: &TeamConfig, tasks: &[Task]) -> Vec<LibraryEntry> {
  config
    .resources(ResourceKind::Pipeline)
    .into_iter()
    .filter_map(|resource| {
      let (version, published_at, body) = published(resource)?;
      let body = &body;
      let names = |indexes: Vec<usize>| {
        indexes
          .into_iter()
          .map(|index| body.steps()[index].name.as_str().into())
          .collect()
      };
      Some(LibraryEntry {
        id: resource.id,
        name: resource.name.as_str().into(),
        slug: resource.slug.as_str().into(),
        stages: levels(body.steps()).into_iter().map(names).collect(),
        step_count: body.steps().len(),
        version,
        used_by: tasks
          .iter()
          .filter(|task| task.details().pipeline == Some(resource.id))
          .count(),
        last_published: published_at,
        // A team project's default pipeline is not kept on this computer.
        default: false,
      })
    })
    .collect()
}

/// The latest version of team pipeline `resource`; `None` without one, or while its body is not a pipeline this
/// computer can read (which [`TeamResource::body`] logs).
pub(super) fn published(resource: &TeamResource) -> Option<(VersionNumber, Timestamp, Pipeline)> {
  let latest = resource.latest.as_ref()?;
  let body = resource.body::<Pipeline>()?.ok()?;
  Some((latest.number, latest.published_at, body))
}

impl Servers {
  /// The versions of pipeline `pipeline` of the project at `root`, newest first: a team project's as its team server
  /// has them ([`Self::team_pipeline_history`]), a local project's as its database keeps them.
  pub async fn pipeline_history(&self, root: &Path, pipeline: ResourceId) -> Result<VersionHistory, Message> {
    let opened = root.to_owned();
    let shared = background::run(move || Ok::<_, StoreError>(ProjectDb::open(&opened)?.sharing()?.is_some()))
      .await?
      .map_err(|error| messages::store_error(&error))?;
    if shared {
      return self.team_pipeline_history(root, pipeline).await;
    }
    let opened = root.to_owned();
    background::run(move || VersionHistory::load_in(&opened, pipeline).map_err(|error| messages::store_error(&error)))
      .await?
  }

  /// Every published version of the team's pipeline `pipeline` of the project at `root`, as the team server has them,
  /// newest first. A version whose body this computer cannot read as a pipeline is left out, and logged. Runs on this
  /// computer pin a team version through its execution bundle, not through these, so none is listed beside them. Why
  /// they could not be read is said of the project's server.
  pub async fn team_pipeline_history(&self, root: &Path, pipeline: ResourceId) -> Result<VersionHistory, Message> {
    let (servers, opened) = (self.clone(), root.to_owned());
    let server = background::run(move || ProjectServer::open(&servers, &opened))
      .await?
      .map_err(|error| unopened(&error))?
      .ok_or_else(|| messages::store_error(&StoreError::NotShared))?;
    let mut read = Vec::new();
    let mut after = None;
    loop {
      let page = server
        .versions(pipeline, after)
        .await
        .map_err(|error| error.message(&server.entry().address))?;
      for version in page.versions {
        match serde_json::from_value::<Pipeline>(version.body) {
          Ok(body) => read.push((version.number, Timestamp::from_unix_millis(version.published.at), body)),
          Err(error) => {
            tracing::warn!(%pipeline, number = version.number.get(), %error, "a team pipeline's version cannot be read");
          }
        }
      }
      match page.next {
        Some(next) => after = Some(next),
        None => return Ok(VersionHistory::of(read, std::collections::BTreeMap::new())),
      }
    }
  }
}

/// Why the project's server could not be opened, before any server is known to name: its database, or a server no
/// longer in the list.
fn unopened(error: &SharingError) -> Message {
  match error {
    SharingError::Store(error) => messages::store_error(error),
    SharingError::ServerMissing => Message::new("sharing.errors.serverMissing"),
    error => {
      tracing::warn!(%error, "the team server of a project could not be opened to read a pipeline's versions");
      messages::interrupted()
    }
  }
}
