//! A run's execution inputs, kept with the run when it is created (`docs/architecture/team-authority-plan.md`, §7):
//! the canonical content that was accepted for it ([`BundleContent`]), how this computer binds that content
//! ([`LocalBindings`]) and what a person here narrowed or consented to ([`LocalConsent`]). The three are kept apart, so
//! local resolution never passes for another publication, and none holds a secret value. A run executes and recovers
//! from them without reading the current task or resource definitions; they never change once kept.

mod bundle;
mod local;
mod resolve;

use thiserror::Error;

pub use self::{
  bundle::{BundleContent, InputsOrigin, RUN_INPUTS_FORMAT, TaskAtStart, TeamAcceptance},
  local::{BoundReference, LocalBindings, LocalConsent, ProfileBinding, TeamMcpBinding},
  resolve::team_mcp_id,
};
use crate::{
  connection::mcp::McpSelectionError,
  id::{ConnectionId, ResourceId},
  name::NameError,
  resource::{StepKey, VersionNumber},
  run::{PinnedVersion, RunSnapshot, bundle::PinnedBody},
  task::LimitsError,
};

/// Why inputs cannot be made, or read back.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InputsError {
  #[error("the run inputs are format {0}, not {RUN_INPUTS_FORMAT}")]
  Format(u32),
  #[error(transparent)]
  Limits(#[from] LimitsError),
  #[error("the {0} cannot be represented in the run inputs")]
  Unrepresentable(&'static str),
  #[error("the task's title: {0}")]
  TaskTitle(NameError),
  #[error("agent profile {0} has no local binding")]
  MissingBinding(ResourceId),
  /// A binding names a profile the inputs do not pin, or a pinned profile twice.
  #[error("agent profile {0} is bound but not pinned once")]
  UnexpectedBinding(ResourceId),
  #[error("the bound MCP servers: {0}")]
  McpServers(McpSelectionError),
  #[error("agent profile {profile} names MCP server {server}, which the inputs do not bind")]
  UnboundMcpServer { profile: ResourceId, server: ConnectionId },
  #[error("{resource} {number} is accepted here but the inputs do not pin it")]
  AcceptedUnpinned {
    resource: ResourceId,
    number: VersionNumber,
  },
  #[error("step `{0}` is narrowed here but runs no agent in the inputs")]
  NarrowedUnknown(StepKey),
  #[error("step `{0}` is narrowed here to more than the inputs permit")]
  NarrowedWider(StepKey),
}

/// A run's execution inputs: canonical content, local bindings and local consent, consistent with each other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunInputs {
  bundle: BundleContent,
  bindings: LocalBindings,
  consent: LocalConsent,
}

impl RunInputs {
  /// The inputs of `bundle` as this computer binds and narrows them: each pinned agent profile bound once, MCP servers
  /// bound that a profile can give its agents, consent given only to pinned versions, and narrowing that never widens a
  /// step's permissions.
  pub fn new(bundle: BundleContent, bindings: LocalBindings, consent: LocalConsent) -> Result<Self, InputsError> {
    check_bindings(&bundle, &bindings)?;
    resolve::mcp_servers(&bindings)?;
    for binding in &bindings.profiles {
      resolve::profile_mcp_servers(&bindings, binding)?;
    }
    check_consent(&bundle, &consent)?;
    Ok(Self {
      bundle,
      bindings,
      consent,
    })
  }

  pub const fn bundle(&self) -> &BundleContent {
    &self.bundle
  }

  pub const fn bindings(&self) -> &LocalBindings {
    &self.bindings
  }

  pub const fn consent(&self) -> &LocalConsent {
    &self.consent
  }

  /// Whether `snapshot` pins exactly the versions these inputs carry; its sources are this computer's and not part of
  /// the inputs.
  pub fn pinned_by(&self, snapshot: &RunSnapshot) -> bool {
    let carried = self.bundle.inputs().snapshot();
    carried.pipeline() == snapshot.pipeline()
      && carried.steps() == snapshot.steps()
      && carried.instructions() == snapshot.instructions()
      && carried.tools() == snapshot.tools()
      && carried.skills() == snapshot.skills()
  }
}

fn check_bindings(bundle: &BundleContent, bindings: &LocalBindings) -> Result<(), InputsError> {
  let pinned: Vec<ResourceId> = bundle
    .inputs()
    .profiles()
    .iter()
    .map(|profile| profile.pinned().resource)
    .collect();
  let mut bound: Vec<ResourceId> = bindings.profiles.iter().map(|binding| binding.resource).collect();
  bound.sort();
  if let Some(pair) = bound.windows(2).find(|pair| pair[0] == pair[1]) {
    return Err(InputsError::UnexpectedBinding(pair[0]));
  }
  if let Some(extra) = bound.iter().find(|resource| !pinned.contains(resource)) {
    return Err(InputsError::UnexpectedBinding(*extra));
  }
  match pinned.iter().find(|resource| !bound.contains(resource)) {
    Some(missing) => Err(InputsError::MissingBinding(*missing)),
    None => Ok(()),
  }
}

fn check_consent(bundle: &BundleContent, consent: &LocalConsent) -> Result<(), InputsError> {
  let inputs = bundle.inputs();
  let pins: Vec<PinnedVersion> = std::iter::once(inputs.pipeline().pinned())
    .chain(inputs.profiles().iter().map(PinnedBody::pinned))
    .chain(inputs.instructions().iter().map(PinnedBody::pinned))
    .chain(inputs.tools().iter().map(PinnedBody::pinned))
    .chain(inputs.skills().iter().map(PinnedBody::pinned))
    .collect();
  if let Some(unpinned) = consent.accepted.iter().find(|accepted| !pins.contains(accepted)) {
    return Err(InputsError::AcceptedUnpinned {
      resource: unpinned.resource,
      number: unpinned.number,
    });
  }
  let resolved = inputs.step_permissions();
  for narrowed in &consent.narrowed {
    let step = resolved
      .iter()
      .find(|resolved| resolved.step == narrowed.step)
      .ok_or_else(|| InputsError::NarrowedUnknown(narrowed.step.clone()))?;
    if !narrowed.permissions.widenings_from(&step.permissions).is_empty() {
      return Err(InputsError::NarrowedWider(narrowed.step.clone()));
    }
  }
  Ok(())
}
