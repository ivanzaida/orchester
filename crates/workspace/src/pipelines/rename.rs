//! Renaming a pipeline from the Pipelines hub's … menu: the display name changes at once; the slug and the versions
//! stay as they are. Another pipeline of the project, archived ones included (restoring it must not make two of a
//! name), may not have the name, nor a name that makes the slug it has. A team project's pipeline is renamed on its
//! team server, whose answer the team's configuration in memory takes (D-042).

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  name::{Name, NameError},
  resource::{ResourceKind, Slug},
};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  messages::{self, Message},
  sharing::{NotTaken, Servers},
  team_config::TeamNotRead,
};

/// Why a pipeline was not renamed.
#[derive(Debug, thiserror::Error)]
pub enum RenameError {
  #[error(transparent)]
  Name(#[from] NameError),
  /// Another pipeline of the project has this name.
  #[error("another pipeline is already named {name}")]
  Taken { name: String, pipeline: String },
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The team's configuration is being read, or could not be read: its pipelines' names are not known.
  #[error(transparent)]
  TeamNotRead(#[from] TeamNotRead),
  /// The team server did not take the new name.
  #[error("the team server did not take the pipeline's new name")]
  Team(NotTaken),
}

impl RenameError {
  /// What a person reads when the pipeline keeps its name.
  pub fn message(&self) -> Message {
    match self {
      Self::Name(NameError::Blank) => Message::new("pipelines.rename.blank"),
      Self::Name(NameError::TooLong { .. }) => {
        Message::new("pipelines.rename.tooLong").with("count", Name::MAX_CHARS.to_string())
      }
      Self::Taken { name, pipeline } => Message::new("pipelines.rename.taken")
        .with("name", name.clone())
        .with("pipeline", pipeline.clone()),
      Self::Store(error) => messages::store_error(error),
      Self::TeamNotRead(unread) => unread.message(),
      Self::Team(refused) => refused.message(),
    }
  }
}

/// Renames the pipeline `id` of the project at `repo_root` to `name`, as the person typed it: trimmed, not blank,
/// not another pipeline's, and returns the name as it was saved. A team project's is renamed on its team server
/// through `servers`. Blocks, on the server's answer too: run it on a worker of the application's runtime.
pub fn rename_in((repo_root, servers): (&Path, &Servers), id: ResourceId, name: &str) -> Result<Name, RenameError> {
  let name = Name::new(name)?;
  let mut db = ProjectDb::open(repo_root)?;
  if db.sharing()?.is_some() {
    return rename_for_team((repo_root, servers), id, name);
  }
  let slug = Slug::from_name(name.as_str());
  let wanted = name.as_str().to_lowercase();
  let archived = db
    .archived_local_resources(ResourceKind::Pipeline)?
    .into_iter()
    .map(|(resource, _)| resource);
  let taken = db
    .kept_resources(ResourceKind::Pipeline)?
    .into_iter()
    .chain(archived)
    .find(|other| {
      other.id() != id && (other.name().as_str().to_lowercase() == wanted || slug.as_ref() == Some(other.slug()))
    });
  if let Some(other) = taken {
    return Err(RenameError::Taken {
      name: name.as_str().to_owned(),
      pipeline: other.name().as_str().to_owned(),
    });
  }
  db.rename_local_resource(id, &name)?;
  Ok(name)
}

/// Renames the team's pipeline `id` to `name` on the team server, unless another of the team's pipelines, archived
/// ones included, has the name or the slug it makes.
fn rename_for_team((root, servers): (&Path, &Servers), id: ResourceId, name: Name) -> Result<Name, RenameError> {
  let config = servers.workspace().team_read(root).loaded()?;
  let slug = Slug::from_name(name.as_str());
  let wanted = name.as_str().to_lowercase();
  let named = config
    .resources(ResourceKind::Pipeline)
    .into_iter()
    .chain(config.archived_of(ResourceKind::Pipeline));
  let taken = named.into_iter().find(|other| {
    let same_slug = slug.as_ref().is_some_and(|slug| slug.as_str() == other.slug);
    other.id != id && (other.name.as_str().to_lowercase() == wanted || same_slug)
  });
  if let Some(other) = taken {
    return Err(RenameError::Taken {
      name: name.as_str().to_owned(),
      pipeline: other.name.as_str().to_owned(),
    });
  }
  servers
    .rename_team_pipeline(root, id, &name)
    .map_err(RenameError::Team)?;
  Ok(name)
}
