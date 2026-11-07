//! A team project's answer: the server's, as `GET …/resources/{resource}/used-by` gives it.

use orchester_core::id::ResourceId;
use orchester_store::SharingLink;

use super::{UsedBy, UsedByError, is_of};
use crate::team_reads::{TeamReadError, TeamSession};

/// The server's answer for `resource` in the session of the project `link` names. The answer of a session that ended
/// while the call was out is not returned.
pub(super) async fn used_by(
  session: Option<&TeamSession>,
  link: &SharingLink,
  resource: ResourceId,
) -> Result<UsedBy, UsedByError> {
  let session = session.ok_or(UsedByError::NoTeamSession)?;
  if !is_of(session, link) {
    return Err(UsedByError::OtherProject);
  }
  if !session.current() {
    return Err(UsedByError::SessionChanged);
  }
  let (client, token) = session.client();
  let answer = client.used_by(token, session.project(), resource).await;
  if !session.current() {
    return Err(UsedByError::SessionChanged);
  }
  Ok(answer.map_err(TeamReadError::from)?)
}
