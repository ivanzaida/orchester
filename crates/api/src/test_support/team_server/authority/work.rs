//! Who the stand-in's `/api/v2` reads name on a task or a module: its assignee and the claim its work runs under, the
//! person reading or someone by name; and the modules [`super::TeamServer::authority_modules`] sets, each with its
//! branch, dates, assignee, claim and forecast. A claim goes stale [`STALE_AFTER_MINUTES`] after its last renewal, as
//! the server's default threshold says.

use orchester_core::time::Timestamp;
use serde_json::{Value, json};

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

/// A claim: who holds it, how long ago its work last renewed it, and whether the server finds it stale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorityClaim {
  pub holder: Person,
  pub renewed_minutes_ago: i64,
  pub stale: bool,
}

impl AuthorityClaim {
  /// The claim on `subject` (`{kind, id}`), its id made from `number`.
  pub(super) fn wire(&self, subject: &Value, number: u32) -> Value {
    let renewed = Timestamp::now().unix_millis() - self.renewed_minutes_ago * 60_000;
    json!({
      "subject": subject, "id": format!("0191c7c0-0000-7000-8000-0000003{number:05x}"), "holder": self.holder.wire(),
      "acquired_at": renewed - 30 * 60_000, "last_execution_heartbeat_at": renewed,
      "stale_at": renewed + STALE_AFTER_MINUTES * 60_000, "stale": self.stale, "parent_module_claim_id": null,
    })
  }
}

/// A module as the stand-in's reads give it, its days `YYYY-MM-DD`; `likely` its forecast's 50% and 85% days, or too
/// little history without them.
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
}

/// The id of the module at `index` of those [`super::TeamServer::authority_modules`] set.
pub(super) fn module_id(index: usize) -> String {
  format!("0191c7c0-0000-7000-8000-0000002{index:05x}")
}

impl AuthorityModule {
  /// The module at `index` as a `ModuleRead`, with `capabilities`.
  pub(super) fn read(&self, index: usize, capabilities: &Value) -> Value {
    let id = module_id(index);
    let at = json!({"by": Person::You.wire(), "at": Timestamp::now().unix_millis() - 21 * 24 * 60 * 60_000});
    let number = u32::try_from(index).unwrap_or_default() + 0x800;
    json!({
      "id": id, "name": self.name, "description": self.description, "start": self.start, "target": self.target,
      "state": "open", "revision": 1, "created": at, "updated": at, "branch": self.branch,
      "assignee": self.assignee.map(Person::wire), "work": null,
      "effective_claim": self.claim.map(|claim| claim.wire(&json!({"kind": "module", "id": id}), number)),
      "capabilities": capabilities,
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
