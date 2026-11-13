//! Page-based creation, including copying a published version, and the Pipelines hub's, which creates the pipeline the
//! person made in its panel. In a team project the new pipeline goes to the team server at once and is kept only once
//! the server made it ([`create_with`]).

use std::path::Path;

use orchester_core::{id::ResourceId, resource::Pipeline, time::Timestamp};
use orchester_store::ProjectDb;

use super::{
  fields::parse_name,
  library::{StarterText, create_with, project_assignables},
  write::WriteError,
};
use crate::{
  messages::{self, Message},
  sharing::Servers,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
  Starter,
  Empty,
  Copy(ResourceId),
}

pub fn create_in(
  (root, servers): (&Path, &Servers),
  name: &str,
  description: &str,
  start: Start,
  starter: &StarterText,
) -> Result<ResourceId, Message> {
  let name = parse_name(name)?;
  let mut db = ProjectDb::open(root).map_err(|e| messages::store_error(&e))?;
  let body = match start {
    Start::Empty => Pipeline::new(Vec::new()).expect("an empty pipeline is a valid body"),
    Start::Starter => starter.pipeline(&project_assignables(&db, (root, servers)).map_err(|e| e.message())?),
    // A team project's pipeline is copied from the team's configuration as this session read it.
    Start::Copy(id) => {
      servers
        .workspace()
        .open_pipeline(root, id)
        .map_err(|error| WriteError::from(error).message())?
        .body
    }
  }
  .with_description(description.to_owned());
  create_with(&mut db, (root, servers), name, &body, Timestamp::now()).map_err(|e| e.message())
}

/// Creates pipeline `name` with `body`, the steps and description the person made in the hub's panel, as its first
/// version: nothing of it is stored before (D-032), and a body the save check refuses is not stored at all.
pub fn create_made_in((root, servers): (&Path, &Servers), name: &str, body: &Pipeline) -> Result<ResourceId, Message> {
  let name = parse_name(name)?;
  let mut db = ProjectDb::open(root).map_err(|e| messages::store_error(&e))?;
  create_with(&mut db, (root, servers), name, body, Timestamp::now()).map_err(|e| e.message())
}
