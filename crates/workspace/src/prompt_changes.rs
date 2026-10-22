//! What changes the text that reaches agents' prompts (ORC-30): publishing an instruction, a profile's role,
//! responsibilities or role instructions, and loading the repository's own agent files. Automation saves drafts of
//! them freely, since a draft reaches no prompt; making such a change take effect is a person's decision, as widening
//! what agents may do is (`crate::app_mcp`'s `person_only`). Otherwise a file an agent read could make it steer every
//! later agent.

use orchester_core::{id::ResourceId, resource::AgentProfile, store::ResourceStore};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::Message;

/// Publishing an instruction: its text reaches every agent it applies to.
pub fn instruction() -> Message {
  Message::new("appMcp.prompts.instruction")
}

/// Turning on the repository's own agent files (AGENTS.md, CLAUDE.md): what the repository says reaches every agent.
pub fn repository_agent_files() -> Message {
  Message::new("appMcp.prompts.repositoryAgentFiles")
}

/// What `new` changes in its agents' prompts over `latest`, the profile's latest published version (before the first:
/// any responsibilities or role instructions).
pub fn profile(latest: Option<&AgentProfile>, new: &AgentProfile) -> Vec<Message> {
  let changed = latest.map_or_else(
    || !new.responsibilities().trim().is_empty() || !new.instructions().as_str().trim().is_empty(),
    |latest| {
      latest.role() != new.role()
        || latest.responsibilities() != new.responsibilities()
        || latest.instructions() != new.instructions()
    },
  );
  if changed {
    vec![Message::new("appMcp.prompts.profile").with("role", new.role().as_str().to_owned())]
  } else {
    Vec::new()
  }
}

/// [`profile`] against the latest published version of profile `id`.
pub fn profile_in(db: &ProjectDb, id: ResourceId, new: &AgentProfile) -> Result<Vec<Message>, StoreError> {
  let latest = db.latest_version::<AgentProfile>(id)?;
  Ok(profile(latest.as_ref().map(|version| version.body()), new))
}
