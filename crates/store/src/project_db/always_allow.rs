//! Always allow of a command (`NV6xJ`): the version of the agent profile used here with the chosen
//! prefix in its Allowed commands is published as the next version, and the profile's unsaved draft,
//! if it has one, gets the prefix too and is based on that version from then on, so it still
//! publishes. A teammate's version that waits for *Use v<N> here* is not built on: what it adds
//! never reaches runs this way, and the version published here supersedes it. One transaction, so
//! neither the version nor the draft can be left half done.

use orchester_core::{
  id::ResourceId,
  permission::{CommandPrefix, category::ClassifiedCommand},
  resource::{AgentProfile, Draft, VersionNumber, publish},
  run::{allowing_command, listing},
  time::Timestamp,
};
use rusqlite::params;

use super::{
  ProjectDb,
  resources::{encode, load_draft, newest_number, used_here},
  shared_versions::stop_waiting,
};
use crate::StoreError;

impl ProjectDb {
  /// Publishes the version of agent profile `profile` used here with `prefix` added to its Allowed commands,
  /// which Always allow must offer for `command`, and adds the prefix to its draft. Returns the version.
  pub fn publish_allowed_command(
    &mut self,
    profile: ResourceId,
    command: &ClassifiedCommand,
    prefix: &CommandPrefix,
    now: Timestamp,
  ) -> Result<VersionNumber, StoreError> {
    let transaction = self.connection.transaction()?;
    let latest = used_here::<AgentProfile>(&transaction, profile)?.ok_or(StoreError::ResourceNotFound(profile))?;
    let newest = newest_number(&transaction, profile)?;
    let permissions = allowing_command(latest.body().permissions(), command, prefix)?;
    let body = latest.body().clone().with_permissions(permissions);
    let version = publish(Draft::new(body, newest, now), newest, now)?;
    transaction.execute(
      "INSERT INTO resource_versions (id, resource_id, number, body, published_at) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        version.id().to_string(),
        profile.to_string(),
        version.number().get(),
        encode(version.body())?,
        now.unix_millis()
      ],
    )?;
    stop_waiting(&transaction, profile, version.number())?;
    if let Some(draft) = load_draft::<AgentProfile>(&transaction, profile)? {
      let body = draft.body().clone();
      // A draft whose commands are no list any more keeps them as the person set them.
      let body = match listing(body.permissions(), prefix) {
        Ok(permissions) => body.with_permissions(permissions),
        Err(_) => body,
      };
      let based_on = match draft.based_on() {
        Some(number) if number == latest.number() => Some(version.number()),
        other => other,
      };
      transaction.execute(
        "UPDATE resource_drafts SET body = ?2, based_on = ?3 WHERE resource_id = ?1",
        params![profile.to_string(), encode(&body)?, based_on.map(VersionNumber::get)],
      )?;
    }
    transaction.commit()?;
    Ok(version.number())
  }
}
