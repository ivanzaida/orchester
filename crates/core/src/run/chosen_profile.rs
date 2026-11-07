//! The agent profile a person chose to run a step with when they returned the run's work to it, for that return only.

use serde::{Deserialize, Serialize};

use super::snapshot::PinnedVersion;
use crate::resource::ModelId;

/// A published version of an agent profile and the model it runs on, which the runner read from that version when the
/// work was returned. The run's snapshot, and with it the pipeline and later runs, keeps its own agents
/// ([`super::Run::executing_agent`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChosenProfile {
  pub profile: PinnedVersion,
  pub model: ModelId,
}
