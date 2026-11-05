//! Who the stand-in's `/api/v2` reads name on a task or a module: its assignee and the claim its work runs under, the
//! person reading or someone by name; and the modules [`super::TeamServer::authority_modules`] sets, each with its
//! branch, dates, assignee, claim and forecast. A claim goes stale [`STALE_AFTER_MINUTES`] after its last renewal, as
//! the server's default threshold says.

use orchester_core::time::Timestamp;
use serde_json::{Value, json};

use super::PendingDelivery;
use crate::test_support::team_server::USER;

/// The server's claim staleness threshold.
pub const STALE_AFTER_MINUTES: i64 = 15;

/// Someone a read names: the person reading, or another member by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Person {
  You,
  Named(&'static str),
}

impl Person {
  /// `{id, display_name}`; another member's id is made from the name, so it is the same in every read.
  pub(super) fn wire(self) -> Value {
    match self {
      Self::You => json!({"id": USER, "display_name": "Bohdan Koval"}),
      Self::Named(name) => {
        let seed = name
          .bytes()
          .fold(0_u32, |sum, byte| sum.wrapping_mul(31).wrapping_add(u32::from(byte)));
        json!({"id": format!("0191c7c0-0000-7000-8000-0000000c{:04x}", seed & 0xffff), "display_name": name})
      }
    }
  }
}

/// A claim: who holds it, how long ago its work last renewed it, whether the server finds it stale, and which of the
/// subject's claims it is: each acquisition and take-over issues a new one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorityClaim {
  pub holder: Person,
  pub renewed_minutes_ago: i64,
  pub stale: bool,
  pub generation: u8,
}

impl AuthorityClaim {
  /// A fresh claim of `holder`, renewed now, the subject's claim after `before`.
  pub(super) const fn taken(holder: Person, before: Option<&Self>) -> Self {
    let generation = match before {
      Some(before) => before.generation.wrapping_add(1),
      None => 0,
    };
    Self {
      holder,
      renewed_minutes_ago: 0,
      stale: false,
      generation,
    }
  }

  /// The id of this claim of the subject numbered `number`.
  pub(super) fn id(&self, number: u32) -> String {
    format!("0191c7c0-0000-7000-8000-00003{:02x}{number:05x}", self.generation)
  }

  /// The claim on `subject` (`{kind, id}`), its id made from `number`.
  pub(super) fn wire(&self, subject: &Value, number: u32) -> Value {
    let renewed = Timestamp::now().unix_millis() - self.renewed_minutes_ago * 60_000;
    json!({
      "subject": subject, "id": self.id(number), "holder": self.holder.wire(),
      "acquired_at": renewed - 30 * 60_000, "last_execution_heartbeat_at": renewed,
      "stale_at": renewed + STALE_AFTER_MINUTES * 60_000, "stale": self.stale, "parent_module_claim_id": null,
    })
  }
}

/// A module as the stand-in's reads give it, its days `YYYY-MM-DD`; `likely` its forecast's 50% and 85% days, or too
/// little history without them; `pending` its delivery to base that is requested and not yet delivered or failed.
#[derive(Clone, Debug)]
pub struct AuthorityModule {
  pub name: &'static str,
  pub description: &'static str,
  pub start: Option<&'static str>,
  pub target: Option<&'static str>,
  pub branch: &'static str,
  pub assignee: Option<Person>,
  pub claim: Option<AuthorityClaim>,
  pub likely: Option<(&'static str, &'static str)>,
  pub pending: Option<PendingDelivery>,
}

/// The id of the module at `index` of those [`super::TeamServer::authority_modules`] set.
pub(super) fn module_id(index: usize) -> String {
  format!("0191c7c0-0000-7000-8000-0000002{index:05x}")
}

/// The number the claims of the module at `index` are made from, past every task's.
pub(super) fn module_number(index: usize) -> u32 {
  u32::try_from(index).unwrap_or_default() + 0x800
}

/// A module's branch as an Admin changed it: the branch, the module's revision after it, and who changed it.
#[derive(Clone, Debug)]
pub(super) struct BranchChange {
  pub branch: String,
  pub revision: u64,
  pub by: Person,
}

impl AuthorityModule {
  /// The module at `index` as a `ModuleRead`, with `capabilities`; its branch as `changed` says, when it changed.
  pub(super) fn read(&self, index: usize, capabilities: &Value, changed: Option<&BranchChange>) -> Value {
    let id = module_id(index);
    let at = Timestamp::now().unix_millis() - 21 * 24 * 60 * 60_000;
    let created = json!({"by": Person::You.wire(), "at": at});
    let updated = changed.map_or_else(
      || created.clone(),
      |changed| json!({"by": changed.by.wire(), "at": Timestamp::now().unix_millis() - 60_000}),
    );
    let (branch, revision) = changed.map_or((self.branch, 1), |changed| (changed.branch.as_str(), changed.revision));
    json!({
      "id": id, "name": self.name, "description": self.description, "start": self.start, "target": self.target,
      "state": "open", "revision": revision, "created": created, "updated": updated, "branch": branch,
      "assignee": self.assignee.map(Person::wire), "work": null,
      "effective_claim": self.claim.map(|claim| claim.wire(&json!({"kind": "module", "id": id}), module_number(index))),
      "capabilities": capabilities,
      "pending_delivery": self.pending.as_ref().map(PendingDelivery::wire),
    })
  }

  /// Its forecast for `remaining` tasks.
  pub(super) fn forecast(&self, remaining: usize) -> Value {
    let outlook = match self.likely {
      Some((likely, safe)) => json!({"kind": "dates", "likely": likely, "safe": safe, "past_horizon": false}),
      None => json!({"kind": "not_enough_history"}),
    };
    json!({"remaining": remaining, "history_days": 14, "history_delivered": 6, "outlook": outlook})
  }
}
