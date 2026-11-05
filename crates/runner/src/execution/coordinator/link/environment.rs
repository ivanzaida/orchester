//! The environment area of the local link: the project's variables, read from the project database at each ask. In
//! the runner's own tests each request and answer crosses as JSON (`executor::link::sent`).

use std::ops::Deref;

use orchester_core::{resource::CommandEnvironment, store::SettingsStore};
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
#[cfg(feature = "test-agent")]
use crate::execution::executor::link::sent;
use crate::execution::executor::link::{
  LinkError,
  environment::{EnvironmentLink, EnvironmentRequest},
};

impl<D: Deref<Target = ProjectDb>> EnvironmentLink for LocalLink<D> {
  fn project_environment(&self, request: &EnvironmentRequest) -> Result<CommandEnvironment, LinkError> {
    #[cfg(feature = "test-agent")]
    sent::sending(request);
    #[cfg(not(feature = "test-agent"))]
    let _ = request;
    let answer = self.0.project_environment().map_err(|error| read_failed(&error));
    #[cfg(feature = "test-agent")]
    sent::sending(&answer);
    answer
  }
}
