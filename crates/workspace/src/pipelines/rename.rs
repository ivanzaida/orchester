//! Renaming a pipeline from the Pipelines hub's … menu: the display name changes at once, without a draft; the slug,
//! the published versions and the draft stay as they are. Another pipeline of the project, archived ones included
//! (restoring it must not make two of a name), may not have the name, nor a name that makes the slug it has.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  name::{Name, NameError},
  resource::{ResourceKind, Slug},
};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::{self, Message};

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
    }
  }
}

/// Renames the pipeline `id` of the project at `repo_root` to `name`, as the person typed it: trimmed, not blank,
/// not another pipeline's. Blocks: run it on the background pool.
pub fn rename_in(repo_root: &Path, id: ResourceId, name: &str) -> Result<(), RenameError> {
  let name = Name::new(name)?;
  let mut db = ProjectDb::open(repo_root)?;
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
  Ok(db.rename_local_resource(id, &name)?)
}
