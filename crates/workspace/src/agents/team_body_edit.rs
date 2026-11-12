//! The edit of a team instruction, skill, tool or profile in the hub's panel (`docs/design/team-configuration-live.md`,
//! rule 6; `Agents · v2` `acFkz` A26–A29): what its fields start from (the latest version read this session) and Save,
//! which publishes the body the fields make as the next version on the team server at once, made on the version the
//! fields were opened on. No draft is kept: Cancel puts back the version's own body. A version someone published first
//! is refused ([`EditRefused::Changed`]), and memory then holds the server's latest, which [`latest`] reads.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, Tool, VersionNumber},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
  background,
  sharing::{
    Servers,
    team_edit::{EditRefused, Publication},
  },
  team_config::TeamConfig,
};

/// The latest version of a team resource as an edit starts from it: its number and its body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Latest<B> {
  pub number: VersionNumber,
  pub body: B,
}

/// What an edit of team resource `id` starts from as `config` holds it now; `None` for a resource `config` does not
/// hold, one with no version yet, and one whose latest body is not a `B` this computer reads.
pub fn latest<B: DeserializeOwned>(config: &TeamConfig, id: ResourceId) -> Option<Latest<B>> {
  let resource = config.live(id)?;
  let body = resource.body::<B>()?.ok()?;
  Some(Latest {
    number: resource.number()?,
    body,
  })
}

/// Publishes `body` as the next version of team resource `id` of the project at `root`, made on version `base`: an
/// instruction, a skill or a tool, whose body is the team's as it is. The number the team server gave it.
pub async fn save<B>(
  servers: &Servers,
  root: &Path,
  (id, base): (ResourceId, VersionNumber),
  body: &B,
) -> Result<VersionNumber, EditRefused>
where
  B: Serialize + Clone + Send + 'static,
{
  let (local, owned) = (root.to_owned(), body.clone());
  let publication = background::run(move || Publication::of_body(&local, id, &owned))
    .await
    .map_err(EditRefused::Failed)??;
  servers.publish_team(root, id, (Some(base), publication)).await
}

/// Publishes `profile` as the next version of team profile `id` of the project at `root`, whose configuration this
/// session read is `config`, made on version `base`, in the team's form ([`Publication::of_profile`]).
pub async fn save_profile(
  servers: &Servers,
  (root, config): (&Path, &TeamConfig),
  (id, base): (ResourceId, VersionNumber),
  profile: &AgentProfile,
) -> Result<VersionNumber, EditRefused> {
  let (local, owned, config) = (root.to_owned(), profile.clone(), config.clone());
  let publication = background::run(move || Publication::of_profile(&local, id, &owned, &config))
    .await
    .map_err(EditRefused::Failed)??;
  servers.publish_team(root, id, (Some(base), publication)).await
}

/// Publishes `tool` as the next version of team tool `id` of the project at `root`, made on version `base`, in the
/// team's form ([`Publication::of_tool`]): the secret it names by an id of this project goes by its name. So an admin's
/// Save also publishes again a version that named its secret by id.
pub async fn save_tool(
  servers: &Servers,
  root: &Path,
  (id, base): (ResourceId, VersionNumber),
  tool: &Tool,
) -> Result<VersionNumber, EditRefused> {
  let (local, owned) = (root.to_owned(), tool.clone());
  let publication = background::run(move || Publication::of_tool(&local, id, &owned))
    .await
    .map_err(EditRefused::Failed)??;
  servers.publish_team(root, id, (Some(base), publication)).await
}
