//! The profiles, reviewers and tools of a team project: what each asks of this computer, from its latest version as
//! the team server has it ([`TeamConfig`]), with what this computer filled in for it read from the project's database.

use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, TeamAgentProfile, TeamReviewer, TeamTool, Tool},
  sharing::{McpNeed, Needs, NeedsError},
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  needs::{Entry, Profile, Reviewer},
  standing::Undecided,
};
use crate::team_config::{NotTeamForm, TeamConfig, TeamResource};

/// The team's profiles, each read from its latest version in the team's form; one whose latest version is not in that
/// form is listed as [`Undecided::NotTeamForm`]. `servers` are the team's MCP servers as they stand.
pub(super) fn profiles(db: &ProjectDb, config: &TeamConfig, servers: &[McpNeed]) -> Result<Vec<Profile>, StoreError> {
  let mut profiles = Vec::new();
  for resource in config.resources(ResourceKind::AgentProfile) {
    let (Some(number), Some(body)) = (resource.number(), resource.body::<TeamAgentProfile>()) else {
      continue;
    };
    let needs = match body {
      Ok(team) => profile_needs(config, resource.id, &team, servers),
      Err(NotTeamForm) => Err(Undecided::NotTeamForm),
    };
    profiles.push(Profile {
      entry: Entry::team(resource, number, needs),
      pick: db.connection_pick(resource.id)?.map(|pick| pick.connection),
    });
  }
  Ok(profiles)
}

/// The team's reviewers. The connection is the pick of the kind the body names; a pick of the other kind reads as
/// none. One whose latest version is not a team reviewer body (one that names a connection, from before ORC-887) is
/// [`Undecided::NotTeamForm`].
pub(super) fn reviewers(db: &ProjectDb, config: &TeamConfig) -> Result<Vec<Reviewer>, StoreError> {
  let mut reviewers = Vec::new();
  for resource in config.resources(ResourceKind::TeamReviewer) {
    let (Some(number), Some(body)) = (resource.number(), resource.body::<TeamReviewer>()) else {
      continue;
    };
    let Ok(reviewer) = body else {
      reviewers.push(Reviewer {
        entry: Entry::team(resource, number, Err(Undecided::NotTeamForm)),
        pick: None,
        decision_pick: None,
      });
      continue;
    };
    let language = reviewer.cli().is_some();
    reviewers.push(Reviewer {
      pick: if language {
        db.connection_pick(resource.id)?.map(|pick| pick.connection)
      } else {
        None
      },
      decision_pick: if language { None } else { db.decision_pick(resource.id)? },
      entry: Entry::team(resource, number, Ok(Needs::reviewer(resource.id, &reviewer))),
    });
  }
  Ok(reviewers)
}

/// What `team`, the profile `profile`, needs: undecided while it names a resource the team archived or an MCP server
/// this configuration does not hold.
fn profile_needs(
  config: &TeamConfig,
  profile: ResourceId,
  team: &TeamAgentProfile,
  servers: &[McpNeed],
) -> Result<Needs, Undecided> {
  if let Some(archived) = uses_archived(config, team) {
    return Err(archived);
  }
  Needs::profile(profile, team, servers).map_err(|NeedsError::McpServerNotGiven(server)| {
    match config.archived(server) {
      Some(found) => Undecided::ChoosesArchivedMcpServer(server, found.name.as_str().to_owned()),
      None => Undecided::ChoosesMcpServer(server),
    }
  })
}

/// A team tool or instruction the profile names that the team archived. A skill is not checked: a run does not refuse
/// one ([`super::archive_blocks_profiles`]).
fn uses_archived(config: &TeamConfig, team: &TeamAgentProfile) -> Option<Undecided> {
  if let Some(found) = first_archived(config, team.tools()) {
    return Some(Undecided::UsesArchivedTool(found.id, found.name.as_str().to_owned()));
  }
  first_archived(config, team.instruction_resources())
    .map(|found| Undecided::UsesArchivedInstruction(found.id, found.name.as_str().to_owned()))
}

fn first_archived<'a>(config: &'a TeamConfig, ids: &[ResourceId]) -> Option<&'a TeamResource> {
  ids.iter().find_map(|id| config.archived(*id))
}

/// The team's tools: each needs a value here for the team secret it names, if any. A latest version published before
/// the team's form, which holds a project secret by id, is undecided: nothing says whether that secret has a value on
/// this computer, and the tool is not assumed ready until an admin publishes it again by name.
pub(super) fn tools(config: &TeamConfig) -> Vec<Entry> {
  let mut tools = Vec::new();
  for resource in config.resources(ResourceKind::Tool) {
    let (Some(number), Some(body)) = (resource.number(), resource.body::<TeamTool>()) else {
      continue;
    };
    let needs = match body {
      Ok(tool) => Ok(Needs::tool(tool.secret().cloned().into_iter().collect())),
      Err(NotTeamForm) => match resource.body::<Tool>() {
        Some(Ok(tool)) if tool.secret().is_some() => Err(Undecided::TeamToolSecretById),
        _ => Err(Undecided::NotTeamForm),
      },
    };
    tools.push(Entry::team(resource, number, needs));
  }
  tools
}
