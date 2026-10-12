//! Page-based creation, including copying a published version.
use std::path::Path;

use orchester_core::{id::ResourceId, resource::Pipeline, store::ResourceStore, time::Timestamp};
use orchester_store::{ProjectDb, StoreError};

use super::{
  data::Assignables,
  fields::parse_name,
  library::{StarterText, create_with},
};
use crate::messages::{self, Message};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
  Starter,
  Empty,
  Copy(ResourceId),
}

pub fn create_in(
  root: &Path,
  name: &str,
  description: &str,
  start: Start,
  starter: &StarterText,
) -> Result<ResourceId, Message> {
  let name = parse_name(name)?;
  let mut db = ProjectDb::open(root).map_err(|e| messages::store_error(&e))?;
  let body = match start {
    Start::Empty => Pipeline::new(Vec::new()).expect("an empty pipeline is a valid draft"),
    Start::Starter => starter.pipeline(&Assignables::load(&db).map_err(|e| messages::store_error(&e))?),
    Start::Copy(id) => db
      .latest_version::<Pipeline>(id)
      .map_err(|e| messages::store_error(&e))?
      .ok_or_else(|| messages::store_error(&StoreError::ResourceNotFound(id)))?
      .body()
      .clone(),
  }
  .with_description(description.to_owned());
  create_with(&mut db, name, &body, Timestamp::now()).map_err(|e| e.message())
}
