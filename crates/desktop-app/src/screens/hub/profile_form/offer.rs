//! What a profile's fields offer to choose from: this computer's agent connections (the CLIs, a local profile's
//! connection, the models and efforts) and, for each list it gives, the team's live resources or the project's.

use orchester_core::{connection::AgentConnection, id::ResourceId, resource::ResourceKind};
use orchester_workspace::team_config::TeamConfig;

use super::ListKind;
use crate::queries::hub::ProfileOffer;

/// A resource the lists offer, by id and name. A local MCP server is offered by its connection's id as a resource id.
pub(in crate::screens) type Named = (ResourceId, String);

/// What the fields offer to choose from.
#[derive(Clone, Debug, Default)]
pub(in crate::screens) struct Offer {
  /// This computer's agent connections: the CLIs, a local profile's connection, the models and efforts.
  pub connections: Vec<AgentConnection>,
  /// What each list offers, by [`ListKind`]: the team's live resources, or the project's that are not archived.
  pub lists: [Vec<Named>; 4],
  /// Names of resources a profile still gives though they are not offered (archived ones), for its tags.
  pub known: Vec<Named>,
}

impl Offer {
  /// What a profile's fields offer: this computer's agent connections (`read`, `None` while it is read) and, by list,
  /// the team's live resources in a team's project (`team`, with the configuration read this session), else the
  /// project's MCP servers, instructions, tools and skills.
  pub(in crate::screens) fn read(read: Option<&ProfileOffer>, team: Option<Option<&TeamConfig>>) -> Self {
    let mut offer = Self::default();
    if let Some(read) = read {
      offer.connections = read.agents.connections.to_vec();
    }
    match (team, read) {
      (Some(config), _) => offer.take_team(config),
      (None, Some(read)) => {
        let servers = read.agents.mcp.iter().map(|server| {
          (
            ResourceId::from_uuid(server.id().as_uuid()),
            server.name().as_str().to_owned(),
          )
        });
        offer.list_mut(ListKind::McpServers).extend(servers);
        offer
          .list_mut(ListKind::Instructions)
          .extend(read.given.instructions.iter().cloned());
        offer.list_mut(ListKind::Tools).extend(read.given.tools.iter().cloned());
        offer
          .list_mut(ListKind::Skills)
          .extend(read.given.skills.iter().cloned());
      }
      (None, None) => {}
    }
    offer
  }

  /// The team's live resources by list, and the names of those it archived, which a profile may still give.
  fn take_team(&mut self, config: Option<&TeamConfig>) {
    let Some(config) = config else {
      return;
    };
    for (kind, of) in [
      (ListKind::Instructions, ResourceKind::Instruction),
      (ListKind::McpServers, ResourceKind::McpServer),
      (ListKind::Tools, ResourceKind::Tool),
      (ListKind::Skills, ResourceKind::Skill),
    ] {
      let live = config.resources(of).into_iter();
      self
        .list_mut(kind)
        .extend(live.map(|resource| (resource.id, resource.name.as_str().to_owned())));
      self.known.extend(
        config
          .archived_of(of)
          .map(|resource| (resource.id, resource.name.as_str().to_owned())),
      );
    }
  }

  /// What the list `kind` offers.
  pub(in crate::screens) fn list_mut(&mut self, kind: ListKind) -> &mut Vec<Named> {
    &mut self.lists[kind.at()]
  }

  /// The name of `id` for its tag: its id while the offer does not name it.
  pub(super) fn name_of(&self, id: ResourceId) -> String {
    self
      .lists
      .iter()
      .flatten()
      .chain(&self.known)
      .find(|(found, _)| *found == id)
      .map_or_else(|| id.to_string(), |(_, name)| name.clone())
  }
}
