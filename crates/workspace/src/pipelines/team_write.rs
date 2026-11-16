//! A team pipeline's save (D-042, D-043): what [`super::write`] checked goes to the team server at once, the new name
//! first, then the steps as the next version, and the team's configuration in memory takes each once the server took
//! it; nothing is written to the project's database. A refusal, or a server that cannot be reached, changes nothing:
//! the edit stays the person's, with why. When the name was taken and
//! the steps were not, the rename stays, and the save says the steps were not saved ([`Stored::steps_not_saved`]).

use std::path::Path;

use super::{
  data::OpenedPipeline,
  write::{Checked, Stored, WriteError},
};
use crate::{messages::Message, sharing::Servers};

/// Sends `checked`, a save of `before`, to the team server through `servers`, and returns the pipeline as memory has
/// it then.
/// One made from a version or a name the pipeline no longer has here is refused before anything is sent
/// (`errors.draftChanged`). Blocks on the server's answers.
pub(super) fn save(
  servers: &Servers,
  root: &Path,
  before: &OpenedPipeline,
  checked: Checked,
) -> Result<Stored, WriteError> {
  let Checked {
    current,
    renamed,
    steps,
  } = checked;
  if current.version != before.version || current.name != before.name {
    return Err(WriteError::NotSaved(vec![Message::new("errors.draftChanged")]));
  }
  if let Some(name) = &renamed {
    servers
      .rename_team_pipeline(root, current.id, name)
      .map_err(WriteError::Team)?;
  }
  let steps_not_saved =
    match steps.map(|steps| servers.publish_team_pipeline(root, current.id, current.version, &steps)) {
      None | Some(Ok(_)) => None,
      Some(Err(refused)) if renamed.is_some() => Some(refused.message()),
      Some(Err(refused)) => return Err(WriteError::Team(refused)),
    };
  Ok(Stored {
    opened: servers.workspace().open_pipeline(root, current.id)?,
    steps_not_saved,
  })
}
