//! What the instruction, skill and tool tools share in a team project (`docs/design/local-and-team-parity.md`): the
//! team's resources of a kind, read from the team's configuration this session read, with their latest versions; the
//! team's profiles and pipelines their fields choose from; and what a save sends, the next version made on the latest
//! one or the team's new resource as its version 1, at once, as the hub's Save and Create do
//! (`agents::team_body_edit`). The team keeps no draft on a computer, so there is nothing else to publish.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind, VersionNumber},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use super::{
  created::{create_team, refused},
  team_profiles,
};
use crate::{
  agents::team_body_edit,
  app_mcp::{
    error::{Missing, ToolError},
    services::Services,
  },
  instructions::data::{PipelineChoice, ProfileChoice, pipeline_choice},
  sharing::team_edit::NewTeamResource,
  skills::editor::PipelineVersion,
  team_config::TeamConfig,
  workspace::OpenProject,
};

/// A team resource with its latest version.
pub(super) struct TeamEntry<B> {
  pub(super) id: ResourceId,
  pub(super) name: Arc<str>,
  pub(super) slug: Arc<str>,
  pub(super) number: VersionNumber,
  pub(super) published_at: Timestamp,
  pub(super) body: B,
}

/// The team's resources of `kind` in `config` that are not archived and whose latest version this computer reads as a
/// `B`, by name.
pub(super) fn entries<B: DeserializeOwned>(config: &TeamConfig, kind: ResourceKind) -> Vec<TeamEntry<B>> {
  config
    .resources(kind)
    .into_iter()
    .filter_map(|resource| {
      let latest = resource.latest.as_ref()?;
      Some(TeamEntry {
        id: resource.id,
        name: resource.name.as_str().into(),
        slug: resource.slug.as_str().into(),
        number: latest.number,
        published_at: latest.published_at,
        body: resource.body::<B>()?.ok()?,
      })
    })
    .collect()
}

/// The team's resource of `kind` in `config` named by its id or else, ignoring case, by its name or slug.
pub(super) fn named<B: DeserializeOwned>(
  config: &TeamConfig,
  (kind, missing): (ResourceKind, Missing),
  named: &str,
) -> Result<TeamEntry<B>, ToolError> {
  let mut all = entries::<B>(config, kind);
  let lowered = named.to_lowercase();
  let at = all
    .iter()
    .position(|entry| entry.id.to_string() == named)
    .or_else(|| {
      all
        .iter()
        .position(|entry| entry.name.to_lowercase() == lowered || entry.slug.to_lowercase() == lowered)
    })
    .ok_or_else(|| ToolError::not_found(missing, named))?;
  Ok(all.swap_remove(at))
}

/// The team's configuration of the project at `open`, whose database is `db`, as this session read it; `None` for a
/// local project and while it was not read, when the lists list this computer's rows alone.
pub(super) fn config_read(
  services: &Services,
  (open, db): (&OpenProject, &ProjectDb),
) -> Result<Option<Arc<TeamConfig>>, ToolError> {
  if db.sharing()?.is_none() {
    return Ok(None);
  }
  Ok(services.workspace.team_read(&open.repo_root).config().cloned())
}

/// The team's profiles in `config`, as an instruction or a skill chooses among them.
pub(super) fn profile_choices(config: &TeamConfig) -> Vec<ProfileChoice> {
  team_profiles::profiles(config)
    .into_iter()
    .map(|profile| ProfileChoice {
      id: profile.id,
      slug: config
        .resource(profile.id)
        .map_or_else(|| profile.id.to_string(), |resource| resource.slug.clone())
        .into(),
      role: profile.body.role().clone(),
    })
    .collect()
}

/// The team's pipelines in `config` with their latest versions.
pub(super) fn pipelines(config: &TeamConfig) -> Vec<PipelineVersion> {
  entries::<Pipeline>(config, ResourceKind::Pipeline)
    .into_iter()
    .map(|entry| ((entry.id, entry.name, entry.number), entry.body))
    .collect()
}

/// The team's pipelines in `config`, as an instruction or a skill chooses among them, each step with the role of the
/// team profile of `profiles` it runs.
pub(super) fn pipeline_choices(config: &TeamConfig, profiles: &[ProfileChoice]) -> Vec<PipelineChoice> {
  pipelines(config)
    .into_iter()
    .map(|((id, name, number), body)| pipeline_choice((id, name, Some(number)), &body, profiles))
    .collect()
}

/// Refused as `draft_changed` when `expected`, the revision a client read, is not `latest`, the number of team resource
/// `id`'s latest version: someone published a version since.
pub(super) fn based_on((id, latest): (ResourceId, VersionNumber), expected: Option<&str>) -> Result<(), ToolError> {
  match expected {
    Some(expected) if expected != latest.get().to_string() => Err(StoreError::DraftChanged(id).into()),
    _ => Ok(()),
  }
}

/// What a save sends to the team server: the next version of a team resource, made on its latest, or a new one.
pub(super) enum Sent<B> {
  Next(ResourceId, VersionNumber, B),
  New(NewTeamResource),
}

/// Sends `sent`, an instruction's or a skill's, whose body is the team's as it is, to the team server of the project at
/// `root`; the resource's id once the server took it.
pub(super) async fn send<B>(services: &Services, root: &Path, sent: Sent<B>) -> Result<ResourceId, ToolError>
where
  B: Serialize + Clone + Send + 'static,
{
  match sent {
    Sent::Next(id, base, body) => {
      team_body_edit::save(&services.servers, root, (id, base), &body)
        .await
        .map_err(|error| refused(&error))?;
      Ok(id)
    }
    Sent::New(new) => create_team(services, root, new).await,
  }
}

/// The team's resources `team` (each with its id) as a list tool lists them, then this computer's own rows `own` that
/// are not among them.
pub(super) fn with_own(team: &[(ResourceId, Value)], own: impl Iterator<Item = (ResourceId, Value)>) -> Vec<Value> {
  let mut listed: Vec<Value> = team.iter().map(|(_, shown)| shown.clone()).collect();
  listed.extend(
    own
      .filter(|(id, _)| !team.iter().any(|(team, _)| team == id))
      .map(|(_, shown)| shown),
  );
  listed
}

/// `shown`, a team resource as the tools show it, marked as the team's, with its `revision`: its latest version's
/// number, which a save's `expected_revision` takes.
pub(super) fn marked(mut shown: Value, number: VersionNumber) -> Value {
  if let Some(fields) = shown.as_object_mut() {
    fields.insert("team".to_owned(), json!(true));
    fields.insert("revision".to_owned(), json!(number.get().to_string()));
  }
  shown
}
