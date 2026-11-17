//! The exact heartbeat (S5, the server's `docs/claim-authority.md`): `POST /api/v2/projects/{project}/heartbeat` renews
//! the claims the executing session names by their exact authority. Renewal keeps no receipt. A task's claim renews
//! while its latest work is still the holder's to carry on (OA-31, decided 2026-11-17): `running`, `succeeded` (waiting
//! to be merged into its module's branch or delivered) or `paused` at a limit. One waiting for a person refuses the
//! whole heartbeat with `409 decision_required [D1]`, one that `failed` or was `cancelled`, or that has no work under
//! the claim, `409 transition_denied`, and neither renews anything. A module claim renews whenever its holder names it
//! from the session that acquired it, for as long as the holder keeps naming it. A team server older than this rule
//! renews only `running` work and refuses a module authority `409 decision_required [D1]`; the desktop then names its
//! running claims alone.

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

/// `{renewed, lost}`: the claims renewed, task and module claims alike, and the authorities that are no longer current:
/// taken over, released, unassigned, or never this session's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Renewed {
  pub renewed: Vec<ClaimRead>,
  pub lost: Vec<AuthorityRef>,
}
