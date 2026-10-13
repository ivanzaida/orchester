//! Presentation of typed local-integration errors; unsupported states keep their specific explanation.
use lurq::app::component::DevtoolsInspectable;
use orchester_core::integration::local::LocalError;
use orchester_workspace::tasks::local_integration::IntegrationError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem(pub &'static str);
impl DevtoolsInspectable for Problem {}

pub fn problem(error: IntegrationError) -> Problem {
  use orchester_exec::local_git::LocalGitError;
  let key = match error {
    IntegrationError::State(state) | IntegrationError::Git(LocalGitError::State(state)) => match state {
      LocalError::Stale => "stale",
      LocalError::AuthorityChanged => "authority",
      LocalError::ForeignOperation => "foreign",
      LocalError::WrongPhase => "phase",
      LocalError::NotChecked => "unchecked",
      LocalError::ApprovalRequired => "approval",
      LocalError::NotVerified => "unverified",
      LocalError::Reconcile => "unknown",
      LocalError::Unsupported => "unsupported",
      LocalError::ConditionalRemovalUnsupported => "conditionalRemoval",
      LocalError::ProcessContainmentUnsupported => "containment",
      LocalError::StoreOwnershipUnsupported => "storageOwnership",
      LocalError::ProcessUncertain => "processUncertain",
      LocalError::Collision => "collision",
      LocalError::Unresolved => "unresolved",
      LocalError::Blank => "blank",
    },
    IntegrationError::Git(LocalGitError::Refused) => "git",
    IntegrationError::Git(_) => "io",
    IntegrationError::Store(_) => "storage",
    IntegrationError::Run => "run",
    IntegrationError::SensitiveMetadata => "metadata",
    IntegrationError::AlreadyIntegrated => "external",
    IntegrationError::NoCheck => "checkMissing",
    IntegrationError::CheckFailed => "checkFailed",
  };
  Problem(match key {
    "stale" => "local_integration.error.stale",
    "authority" => "local_integration.error.authority",
    "foreign" => "local_integration.error.foreign",
    "phase" => "local_integration.error.phase",
    "unchecked" => "local_integration.error.unchecked",
    "approval" => "local_integration.error.approval",
    "unverified" => "local_integration.error.unverified",
    "unknown" => "local_integration.error.unknown",
    "unsupported" => "local_integration.error.unsupported",
    "conditionalRemoval" => "local_integration.error.conditionalRemoval",
    "containment" => "local_integration.error.containment",
    "storageOwnership" => "local_integration.error.storageOwnership",
    "processUncertain" => "local_integration.error.processUncertain",
    "collision" => "local_integration.error.collision",
    "unresolved" => "local_integration.error.unresolved",
    "blank" => "local_integration.error.blank",
    "git" => "local_integration.error.git",
    "storage" => "local_integration.error.storage",
    "run" => "local_integration.error.run",
    "metadata" => "local_integration.error.metadata",
    "external" => "local_integration.externalBody",
    "checkMissing" => "local_integration.error.checkMissing",
    "checkFailed" => "local_integration.error.checkFailed",
    _ => "local_integration.error.io",
  })
}
