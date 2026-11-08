//! The two changes of an admin's edit of a team MCP server that take a version of their own back out of the outbox:
//! taking it back as the draft it was written as, still edited from the version it was edited from (so a version the
//! team published meanwhile can be told apart from what the person changed), and rebasing that draft on the team's
//! newer version ([`crate::agents::team_mcp_conflict::rebase`]). Neither runs while a send of the outbox is under way,
//! and both write the draft through the resource's save lane ([`Servers::save_lane`]), so an autosave that started
//! before cannot overwrite what they wrote.

use std::path::Path;

use orchester_core::{
  id::{ResourceId, VersionId},
  resource::{TeamMcpServer, VersionNumber},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  agents::{
    team_mcp_conflict::{self, Based},
    team_mcp_merge::{McpField, Pick},
  },
  background,
  messages::{self, Message},
  sharing::{Servers, send_lane::between_sends},
};

impl Servers {
  /// Takes `version` of a team MCP server, published here and not numbered by the team's server (refused, met by a
  /// newer version, or waiting), back as the resource's draft, edited from the version it was edited from. Nothing is
  /// sent. Fails with what to tell the person.
  pub async fn reopen_mcp_draft(&self, repo_root: &Path, version: VersionId) -> Result<(), Message> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    background::run(move || {
      let lane = servers.send_lane(&root);
      let _between = between_sends(&lane)?;
      let mut db = ProjectDb::open(&root).map_err(|error| messages::store_error(&error))?;
      // A version the outbox does not hold has no resource to order against; the store says why it cannot be taken
      // back.
      let resource = db
        .waiting_publishes()
        .map_err(|error| messages::store_error(&error))?
        .into_iter()
        .find(|waiting| waiting.version == version)
        .map(|waiting| waiting.resource);
      let mut reopen = || {
        db.reopen_draft_on_its_base::<TeamMcpServer>(version, Timestamp::now())
          .map_err(|error| messages::store_error(&error))
      };
      match resource {
        Some(resource) => servers.save_lane(&root, resource).run_newest(reopen),
        None => reopen(),
      }
    })
    .await??;
    self.shared_changed();
    Ok(())
  }

  /// *Rebase my draft on vN*: the draft of the team MCP server `resource`, or the version of the person's own in the
  /// outbox that met the team's (`outbox`), becomes the merge with the team's version `expected`, with `picks` for the
  /// fields both changed ([`team_mcp_conflict::rebase`]). The fields the form shows again, and the version they are
  /// edited from. Fails with what to tell the person; nothing is changed then, unless the outbox's version was already
  /// taken back as the draft.
  pub async fn rebase_mcp_draft(
    &self,
    repo_root: &Path,
    resource: ResourceId,
    (outbox, expected): (Option<VersionId>, VersionNumber),
    picks: Vec<(McpField, Pick)>,
  ) -> Result<Based, Message> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let based = background::run(move || {
      let lane = servers.send_lane(&root);
      let _between = between_sends(&lane)?;
      servers.save_lane(&root, resource).run_newest(|| {
        team_mcp_conflict::rebase(&root, resource, (outbox, expected), &picks)
          .map_err(team_mcp_conflict::RebaseError::message)
      })
    })
    .await??;
    self.shared_changed();
    Ok(based)
  }
}
