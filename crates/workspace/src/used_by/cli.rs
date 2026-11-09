//! The CLI kind a profile that uses a resource runs, for the mark a list draws at the profile's row.

use std::path::Path;

use orchester_core::{connection::AgentProvider, id::ResourceId, resource::TeamAgentProfile};

use crate::workspace::Workspace;

impl Workspace {
  /// The CLI kind of the team profile `profile` of the team project at `root`, as its latest version says in the team's
  /// configuration read this session; `None` while it was not read, for a profile it does not hold, and for one whose
  /// latest version is not in the team's form, which names no CLI.
  pub fn team_profile_cli(&self, root: &Path, profile: ResourceId) -> Option<AgentProvider> {
    let read = self.team_read(root);
    let resource = read.config()?.resource(profile)?;
    // A version that is not in the team's form is `Undecided` for readiness; here it only has no mark to draw.
    resource.body::<TeamAgentProfile>()?.ok().map(|team| team.cli())
  }
}
