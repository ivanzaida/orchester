//! How often an invite accepted here is asked for (N1 security re-review 2). Each ask takes an identity token for this
//! computer, and the cloud gives a computer 60 an hour, so a team server that answers *waiting* forever must not use
//! them up: while the join waits for approval it is asked again on its own after 2 minutes, then 4, then 8, and not
//! after the approval expires; after that only when the person asks (*Try again*), and never more than [`MAX_ASKS`]
//! times in all.

use orchester_core::{id::InviteId, time::Timestamp};
use orchester_store::AcceptedInvite;

use super::{Accepted, Cloud, CloudError};

/// The most this computer asks for one invite, the person's asks included.
pub const MAX_ASKS: u32 = 10;

/// How long the asks on their own wait, one after another, while the join waits for approval.
const BACKOFF_MS: [i64; 3] = [2 * MINUTE_MS, 4 * MINUTE_MS, 8 * MINUTE_MS];
const MINUTE_MS: i64 = 60 * 1000;

/// Who asks for an invite accepted here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Asker {
  /// The person: accepting it, or *Try again*.
  Person,
  /// The check while the join waits for approval ([`Cloud::check_accepted`]).
  Check,
}

/// What asking again on its own came to for one invite accepted here.
#[derive(Debug)]
pub struct Checked {
  pub invite: InviteId,
  pub server_name: String,
  pub project_name: Option<String>,
  pub outcome: Result<Accepted, CloudError>,
}

impl Cloud {
  /// Asks again, at `now`, for each invite accepted here whose next ask on its own is due.
  pub async fn check_accepted(&self, now: Timestamp) -> Result<Vec<Checked>, CloudError> {
    let due: Vec<_> = self
      .accepted_invites()
      .await?
      .into_iter()
      .filter(|kept| waits(kept, now) && kept.next_check_at.is_some_and(|at| at <= now))
      .collect();
    let mut checked = Vec::with_capacity(due.len());
    for kept in due {
      let (invite, server_name, project_name) = (kept.invite, kept.server_name.clone(), kept.project_name.clone());
      checked.push(Checked {
        invite,
        server_name,
        project_name,
        outcome: self.join_accepted(kept, Asker::Check, now).await,
      });
    }
    Ok(checked)
  }
}

/// Whether `kept` waits for an approval that has not expired by `now` and is asked again on its own.
pub fn waits(kept: &AcceptedInvite, now: Timestamp) -> bool {
  kept.approval.is_some() && kept.next_check_at.is_some() && approval_open(kept, now)
}

/// Whether this computer still asks for `kept` at all.
pub const fn may_ask(kept: &AcceptedInvite) -> bool {
  kept.asks < MAX_ASKS
}

/// Whether the approval `kept` waits for has not expired by `now`.
pub(super) fn approval_open(kept: &AcceptedInvite, now: Timestamp) -> bool {
  kept.approval.is_some() && kept.approval_expires_at.is_some_and(|at| at > now)
}

/// `kept` counting one more ask by `asker`, before it is made, with nothing due on its own while it runs.
pub(super) fn asking(kept: AcceptedInvite, asker: Asker) -> AcceptedInvite {
  let checks = match asker {
    Asker::Person => 0,
    Asker::Check => kept.checks + 1,
  };
  AcceptedInvite {
    asks: kept.asks + 1,
    checks,
    next_check_at: None,
    needs_code: false,
    ..kept
  }
}

/// `kept` after an ask at `now` that did not end it: asked again on its own on the backoff while its approval is open.
pub(super) fn scheduled(kept: AcceptedInvite, now: Timestamp) -> AcceptedInvite {
  let next = BACKOFF_MS
    .get(kept.checks as usize)
    .map(|wait| Timestamp::from_unix_millis(now.unix_millis() + wait))
    .filter(|at| may_ask(&kept) && kept.approval.is_some() && kept.approval_expires_at.is_some_and(|end| *at < end));
  AcceptedInvite {
    next_check_at: next,
    ..kept
  }
}
