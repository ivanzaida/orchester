//! Archiving and restoring a pipeline from the Pipelines hub, which a local and a team project share: a team project's
//! pipeline is archived and restored on its team server ([`super::archive_resource`]: the runs that pin it stop on
//! every computer when the stream says so), a local project's in its database, with the runs it stops
//! ([`crate::local_archive`]). Archiving stops work, so it is a person's alone: the hub asks only after a person
//! confirmed.

use std::path::Path;

use orchester_core::{id::ResourceId, run::Answerer};
use orchester_store::{ProjectDb, StoreError};

use super::{NotArchived, NotRestored};
use crate::{
  background,
  local_archive::{self, Archivable},
  messages::{self, Message},
  runner::Runners,
  sharing::Servers,
};

impl Servers {
  /// Archives the pipeline `pipeline` of the project at `root`, as a person confirmed. What it answers is what to tell
  /// the person when some of the runs that use it could not be stopped here.
  pub async fn archive_pipeline(
    &self,
    runners: &Runners,
    root: &Path,
    pipeline: ResourceId,
  ) -> Result<Option<Message>, NotArchived> {
    if shared(root).await.map_err(NotArchived::Failed)? {
      self.archive_resource(root, pipeline, Answerer::Person).await?;
      return Ok(None);
    }
    let archived = local_archive::archive(runners, root, Archivable::Resource(pipeline))
      .await
      .map_err(NotArchived::Failed)?;
    Ok(archived.left_running_message())
  }

  /// Brings the archived pipeline `pipeline` of the project at `root` back as it was; `Err` says why it stays archived.
  pub async fn restore_pipeline(&self, root: &Path, pipeline: ResourceId) -> Result<(), Message> {
    if !shared(root).await? {
      return local_archive::restore(root, Archivable::Resource(pipeline)).await;
    }
    self
      .restore_resource(root, pipeline)
      .await
      .map_err(|refused| match refused {
        NotRestored::Refused { code } if code == "not_admin" => Message::new("sharing.errors.notAdmin"),
        NotRestored::Refused { code } => Message::new("sharing.edit.refused.other").with("code", code),
        NotRestored::Failed(message) => message,
      })
  }
}

/// Whether the project at `root` is shared with a team.
async fn shared(root: &Path) -> Result<bool, Message> {
  let root = root.to_owned();
  background::run(move || Ok::<_, StoreError>(ProjectDb::open(&root)?.sharing()?.is_some()))
    .await?
    .map_err(|error| messages::store_error(&error))
}
