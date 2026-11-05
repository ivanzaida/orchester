//! The exact heartbeat (S5, the server's `docs/claim-authority.md`): `POST /api/v2/projects/{project}/heartbeat` renews
//! the claims the executing session names by their exact authority. Renewal keeps no receipt. A claim renews only while
//! its latest work is `running` (D1): one waiting for a person or for review refuses the whole heartbeat with
//! `409 decision_required [D1]`, any other phase with `409 transition_denied`, and neither renews anything.

use serde::{Deserialize, Serialize};

use super::{AuthorityRef, ClaimRead};

/// The most authorities one heartbeat may name.
pub const MAX_HEARTBEAT_AUTHORITIES: usize = 256;

/// `{authorities}`: the exact authorities the caller's session renews.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactHeartbeat {
  pub authorities: Vec<AuthorityRef>,
}

/// `{renewed, lost}`: the claims renewed, and the authorities that are no longer current: taken over, released,
/// unassigned, or never this session's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Renewed {
  pub renewed: Vec<ClaimRead>,
  pub lost: Vec<AuthorityRef>,
}
