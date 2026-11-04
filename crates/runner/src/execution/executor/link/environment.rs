//! The environment area of the coordinator link: the variables Project settings › Environment sets, names and values
//! as the project stores them. They are no secrets (`orchester_core::resource::CommandEnvironment`); secret values
//! never travel here, they come from the executor computer's keychain. The executor asks for them at each use, as it
//! read them before, and hands them to its computer's resources (`crate::computer::ComputerResources::environment`),
//! which make of them the variables its agents and command tools start with there.

use orchester_core::resource::CommandEnvironment;
use serde::{Deserialize, Serialize};

use super::LinkError;

/// The coordinator's answer about the project's environment.
pub trait EnvironmentLink {
  /// The variables the project sets for every agent CLI and command tool its runs start.
  fn project_environment(&self, request: &EnvironmentRequest) -> Result<CommandEnvironment, LinkError>;
}

/// The project's environment: the project is the link's own, so the request names nothing else.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentRequest {}

/// The project's variables, asked through `link`.
pub fn project_environment(link: &(impl EnvironmentLink + ?Sized)) -> Result<CommandEnvironment, LinkError> {
  link.project_environment(&EnvironmentRequest {})
}
