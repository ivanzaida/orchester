//! What publishing a draft would widen in what agents may do, by the same rules the setup tools apply when automation
//! publishes it: a profile's permissions, account and MCP servers, a pipeline that is not provably as strict, a
//! command tool's program, arguments, exit codes or variables. Instructions and skills widen nothing.

use orchester_core::{
  connection::AgentConnection,
  id::ResourceId,
  orchestrator::change::Publishing,
  resource::{AgentProfile, Pipeline, Tool, Version},
  store::{ConnectionStore, ResourceStore},
};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::Message;

/// What publishing the draft of `resource`, of kind `kind`, would widen; nothing without a draft.
pub(super) fn of_draft(db: &ProjectDb, kind: Publishing, resource: ResourceId) -> Result<Vec<Message>, StoreError> {
  match kind {
    Publishing::Profile => profile(db, resource),
    Publishing::Pipeline => {
      let Some(draft) = db.draft::<Pipeline>(resource)? else {
        return Ok(Vec::new());
      };
      let latest = db.latest_version::<Pipeline>(resource)?;
      Ok(crate::pipelines::widening::publish_reasons(
        latest.as_ref().map(Version::body),
        draft.body(),
      ))
    }
    Publishing::Tool => {
      let Some(draft) = db.draft::<Tool>(resource)? else {
        return Ok(Vec::new());
      };
      let latest = db.latest_version::<Tool>(resource)?;
      Ok(crate::tools::widening::changes(
        latest.as_ref().map(Version::body),
        draft.body(),
        "appMcp.projectTools.widens.firstVersion",
      ))
    }
    // Nothing they publish widens what agents may do (`Publishing::can_widen`); a tool test fails when one of their
    // tools gains a `person_only` rule, so this cannot fall behind it unnoticed.
    Publishing::Instruction | Publishing::Skill => Ok(Vec::new()),
  }
}

/// What the draft of profile `resource` widens over its latest version; nothing while its connection is gone, which
/// publishing refuses anyway.
fn profile(db: &ProjectDb, resource: ResourceId) -> Result<Vec<Message>, StoreError> {
  let Some(draft) = db.draft::<AgentProfile>(resource)? else {
    return Ok(Vec::new());
  };
  let connection = db
    .connection(draft.body().connection())?
    .and_then(|connection| AgentConnection::try_from(connection).ok());
  match connection {
    Some(connection) => crate::agents::widening::reasons(db, Some(resource), draft.body(), connection.provider()),
    None => Ok(Vec::new()),
  }
}
