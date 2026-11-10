//! What a team pipeline's steps run, read from the team's configuration as this session read it
//! (`docs/design/team-configuration-live.md`): each team profile as it runs on the connection this computer picked (or
//! the only one of its CLI), and each team tool. Nothing of the team's is read from the project database; only what
//! this computer filled in (the connections and their picks) is.

use orchester_core::{
  resource::{AgentProfile, ResourceKind, TeamAgentProfile, Tool},
  store::{ConnectionStore, McpServerStore},
};
use orchester_store::{ProjectDb, StoreError};

use super::data::{Assignables, ProfileChoice, ToolChoice};
use crate::{agents::team_profile, team_config::TeamConfig};

impl Assignables {
  /// What the steps of the team's pipelines in `config` run on this computer, with its connections and picks from `db`.
  /// A profile or tool whose latest version is not in its kind's team form is left out: a step that uses it reads as
  /// one whose profile or tool is not published, as readiness says.
  pub fn of_team(db: &ProjectDb, config: &TeamConfig) -> Result<Self, StoreError> {
    let connections = db.connections()?;
    let sheets = team_profile::sheets(db, config)?;
    let mut profiles = Vec::new();
    for resource in config.resources(ResourceKind::AgentProfile) {
      let (Some(number), Some(Ok(team))) = (resource.number(), resource.body::<TeamAgentProfile>()) else {
        continue;
      };
      let picked = sheets
        .iter()
        .find(|(id, _)| *id == resource.id)
        .and_then(|(_, sheet)| sheet.runs_on())
        .map_or_else(|| AgentProfile::unresolved_connection(resource.id), |choice| choice.id);
      let body = AgentProfile::from_team(&team, picked);
      profiles.push(ProfileChoice::of(
        (resource.id, resource.name.as_str()),
        number,
        &body,
        &connections,
      ));
    }
    let tools = config
      .resources(ResourceKind::Tool)
      .into_iter()
      .filter_map(|resource| match (resource.number(), resource.body::<Tool>()) {
        (Some(number), Some(Ok(tool))) => Some(ToolChoice::of((resource.id, resource.name.as_str()), number, &tool)),
        _ => None,
      })
      .collect();
    Ok(Self {
      profiles,
      draft_profiles: Vec::new(),
      tools,
      mcp_servers: db.mcp_servers()?,
    })
  }
}
