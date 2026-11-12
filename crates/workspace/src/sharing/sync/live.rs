//! The team's configuration as the stream keeps it in memory (`docs/design/team-configuration-live.md`): read whole
//! from the server's resource routes each time the stream opens, then kept current by its events — a published
//! version reads that resource's latest version, an archive or a restore follows the mark, a rename the name. Only a
//! resource's latest version is read: an older one is never needed, so one in a form this computer cannot read costs
//! nothing. A server that cannot be reached leaves what was read this session in place, said unreachable.

use std::path::Path;

use orchester_api::{Event, SharedResource};
use orchester_core::{name::Name, resource::VersionNumber, time::Timestamp};

use super::stream::Source;
use crate::{
  sharing::SharingError,
  team_config::{TeamConfig, TeamResource, TeamVersion, Unreached},
};

/// Reads the project's whole configuration from the server and keeps it for `root`. Fails, keeping what was read
/// before, when the server does not answer.
pub(super) async fn fill(source: &impl Source, root: &Path) -> Result<(), SharingError> {
  let shared = source.resources().await?;
  let mut resources = Vec::with_capacity(shared.len());
  for resource in &shared {
    let latest = latest(source, resource).await?;
    resources.extend(TeamResource::of(resource, latest));
  }
  tracing::info!(resources = resources.len(), "the team's configuration was read");
  source.configs().fill(root, TeamConfig::new(resources));
  Ok(())
}

/// The server of `root` could not be reached for `error`: said so, keeping what was read this session.
pub(super) fn unreached(source: &impl Source, root: &Path, error: &SharingError) {
  let unreached = Unreached {
    reason: source.unreached(error),
    at: Timestamp::now(),
  };
  source.configs().unreached(root, unreached);
  source.changed();
}

/// Follows `event` in the configuration read for `root`, when it is one of the configuration's; whether it changed.
/// A published version reads the resource's latest version, which fails when the server does not answer.
pub(super) async fn follow(source: &impl Source, root: &Path, event: &Event) -> Result<bool, SharingError> {
  let configs = source.configs();
  Ok(match event {
    Event::ResourcePublished(delta) => {
      let resource = &delta.change.resource;
      let latest = latest(source, resource).await?;
      match TeamResource::of(resource, latest) {
        Some(kept) => configs.update(root, |config| {
          config.keep(kept);
          true
        }),
        None => false,
      }
    }
    Event::ResourceArchived(delta) => configs.update(root, |config| {
      config.mark_archived(delta.change.resource, delta.change.archived)
    }),
    Event::ResourceRenamed(delta) => match Name::new(&delta.change.name) {
      Ok(name) => configs.update(root, |config| config.rename(delta.change.resource, name)),
      Err(_) => {
        tracing::warn!(resource = %delta.change.resource, "the server renamed a resource to a name this computer cannot take");
        false
      }
    },
    _ => false,
  })
}

/// The latest version of `resource`, read alone: the page after the version before it.
async fn latest(source: &impl Source, resource: &SharedResource) -> Result<Option<TeamVersion>, SharingError> {
  let Some(number) = resource.latest else {
    return Ok(None);
  };
  let after = number
    .get()
    .checked_sub(1)
    .and_then(std::num::NonZeroU32::new)
    .map(VersionNumber::new);
  let page = source.versions(resource.id, after).await?;
  Ok(
    page
      .versions
      .into_iter()
      .max_by_key(|version| version.number)
      .map(TeamVersion::from),
  )
}
