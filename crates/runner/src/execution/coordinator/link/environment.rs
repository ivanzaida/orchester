//! The environment area of the local link: the project's variables, read from the project database at each ask.

use std::ops::Deref;

use orchester_core::{resource::CommandEnvironment, store::SettingsStore};
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
use crate::execution::executor::link::{
  LinkError,
  environment::{EnvironmentLink, EnvironmentRequest},
};

impl<D: Deref<Target = ProjectDb>> EnvironmentLink for LocalLink<D> {
  fn project_environment(&self, _: &EnvironmentRequest) -> Result<CommandEnvironment, LinkError> {
    self.0.project_environment().map_err(|error| read_failed(&error))
  }
}
