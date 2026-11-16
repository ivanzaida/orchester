//! The stand-in's module deliveries to base (`module-branches.md` of the platform, "Delivery", S7c and S12): an Admin's
//! `delivery.authorize`, the module holder's `delivery.request` and `delivery.report`, and the pending delivery a
//! module's reads carry. With the automatic merge off, a request that cites no authorization waits for the Admin
//! (`awaiting_admin`, no effect); the Admin's authorization of exactly that binding turns it to `admin`, and the
//! holder's next request of it gets the `deliver_module` effect. An Admin may still authorize first, for a request to
//! cite; with the automatic merge on a request is `automatic` at once.

use orchester_core::time::Timestamp;
use serde_json::{Value, json};

use super::{
  Authority, Person, Standing,
  commands::{Refused, module_holder, module_index},
  lifecycle,
  work::{module_id, module_number},
};

/// How a pending delivery is authorized (`PendingAuthorization`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Authorized {
  /// It waits for an Admin's `delivery.authorize`.
  AwaitingAdmin,
  /// The Admin's `delivery.authorize` under this operation allowed it.
  Admin(String),
  /// The workspace's automatic merge allowed it.
  Automatic,
}

/// A module's delivery that is requested and neither reported nor failed: who asked and when (milliseconds since the
/// Unix epoch), the commits it takes, how it is authorized, and whether the holder was given the merge.
#[derive(Clone, Debug)]
pub struct PendingDelivery {
  pub requested_by: Person,
  pub requested_at: i64,
  pub source_commit: String,
  pub base_commit: String,
  pub authorized: Authorized,
  pub merging: bool,
}

impl PendingDelivery {
  /// A request by `by` `minutes` ago of the module branch at `source_commit` onto `base_commit`, waiting for an Admin.
  pub fn awaiting_admin(by: Person, minutes: i64, (source_commit, base_commit): (&str, &str)) -> Self {
    Self {
      requested_by: by,
      requested_at: Timestamp::now().unix_millis() - minutes * 60_000,
      source_commit: source_commit.to_owned(),
      base_commit: base_commit.to_owned(),
      authorized: Authorized::AwaitingAdmin,
      merging: false,
    }
  }

  /// `PendingDeliveryRead`.
  pub(super) fn wire(&self) -> Value {
    let authorization = match self.authorized {
      Authorized::AwaitingAdmin => "awaiting_admin",
      Authorized::Admin(_) => "admin",
      Authorized::Automatic => "automatic",
    };
    json!({"requested_by": self.requested_by.wire(), "requested_at": self.requested_at,
      "source_commit": self.source_commit, "base_commit": self.base_commit, "authorization": authorization})
  }

  fn binds(&self, (source, base): (&str, &str)) -> bool {
    self.source_commit == source && self.base_commit == base
  }
}

/// An Admin's authorization that no request cited yet.
#[derive(Clone, Debug)]
pub(super) struct KeptAuthorization {
  operation: String,
  module: usize,
  claim: String,
  branch: String,
  source_commit: String,
  base_commit: String,
}

/// What a delivery command names: the module's index, its claim and branch now, and the commits asked of it.
struct Binding<'a> {
  index: usize,
  claim: Option<String>,
  branch: String,
  commits: (&'a str, &'a str),
}

/// The delivery commands of a module: an Admin's authorization, its holder's request and the holder's report.
pub(super) fn deliver(
  authority: &mut Authority,
  (operation, kind): (&str, &str),
  command: &Value,
) -> Result<Value, Refused> {
  let subject = match kind {
    "delivery.authorize" => &command["subject"],
    _ => &command["authority"]["subject"],
  };
  let index = module_index(authority, &subject["id"]).ok_or((404, "not_found"))?;
  let source = command["source_commit"].as_str().unwrap_or_default();
  let base = command["base_commit"].as_str().unwrap_or_default();
  let binding = Binding {
    index,
    claim: module_claim(authority, index),
    branch: branch_now(authority, index),
    commits: (source, base),
  };
  if kind == "delivery.authorize" {
    return authorize(authority, operation, command, &binding);
  }
  if module_holder(authority, &json!({"kind": "module", "id": subject["id"]})) != Some(Person::You) {
    return Err((409, "claim_lost"));
  }
  match kind {
    "delivery.request" => request(authority, command, &binding),
    _ => report(authority, command, &binding),
  }
}

/// `delivery.authorize`: the waiting request of exactly this binding turns to `admin`; otherwise the authorization is
/// kept for a later request to cite.
fn authorize(
  authority: &mut Authority,
  operation: &str,
  command: &Value,
  binding: &Binding<'_>,
) -> Result<Value, Refused> {
  if authority.standing == Standing::Member {
    return Err((403, "forbidden"));
  }
  if binding.claim.is_none() || command["claim_id"].as_str() != binding.claim.as_deref() {
    return Err((409, "claim_lost"));
  }
  checked(command, binding)?;
  let pending = pending_mut(authority, binding.index);
  if let Some(waiting) =
    pending.filter(|pending| pending.binds(binding.commits) && pending.authorized == Authorized::AwaitingAdmin)
  {
    waiting.authorized = Authorized::Admin(operation.to_owned());
    return Ok(answer(binding, ("requested", json!(operation)), &[]));
  }
  authority.authorizations.push(KeptAuthorization {
    operation: operation.to_owned(),
    module: binding.index,
    claim: binding.claim.clone().unwrap_or_default(),
    branch: binding.branch.clone(),
    source_commit: binding.commits.0.to_owned(),
    base_commit: binding.commits.1.to_owned(),
  });
  Ok(answer(binding, ("authorized", json!(operation)), &[]))
}

/// `delivery.request` by the holder: on a cited authorization or the automatic merge it merges; otherwise it waits for
/// the Admin, or, already waiting, is answered as it stands.
fn request(authority: &mut Authority, command: &Value, binding: &Binding<'_>) -> Result<Value, Refused> {
  checked(command, binding)?;
  let cited = command["authorization_operation_id"].as_str().map(str::to_owned);
  let automatic = authority.policy.1;
  let open = pending_slot(authority, binding.index).as_ref().map(|open| {
    let stands = (open.binds(binding.commits), open.merging);
    (stands, open.authorized.clone())
  });
  let authorized = match (open, &cited) {
    (Some(((false, _) | (_, true), _)), _) => return Err((409, "transition_denied")),
    (Some((_, Authorized::AwaitingAdmin)), Some(_)) => return Err((403, "delivery_not_authorized")),
    (Some((_, Authorized::Admin(by))), Some(cited)) if by != *cited => return Err((403, "delivery_not_authorized")),
    (Some((_, Authorized::AwaitingAdmin)), None) if automatic => Authorized::Automatic,
    (Some((_, authorized)), _) => authorized,
    (None, Some(cited)) if take_authorization(authority, cited, binding) => Authorized::Admin(cited.clone()),
    (None, Some(_)) => return Err((403, "delivery_not_authorized")),
    (None, None) if automatic => Authorized::Automatic,
    (None, None) => Authorized::AwaitingAdmin,
  };
  let slot = pending_slot(authority, binding.index);
  let delivery = slot.get_or_insert_with(|| PendingDelivery::awaiting_admin(Person::You, 0, binding.commits));
  delivery.authorized = authorized;
  let operation = match &delivery.authorized {
    Authorized::Admin(operation) => json!(operation),
    Authorized::AwaitingAdmin | Authorized::Automatic => Value::Null,
  };
  if delivery.authorized == Authorized::AwaitingAdmin {
    return Ok(answer(binding, ("requested", operation), &[]));
  }
  delivery.merging = true;
  let (module, (source, base)) = (module_id(binding.index), binding.commits);
  let effect = json!({"kind": "deliver_module", "module_id": module, "branch": binding.branch,
    "source_commit": source, "base_commit": base});
  Ok(answer(binding, ("requested", operation), &[effect]))
}

/// `delivery.report` of the requested delivery of exactly these commits: it ends it. One that still waits was never
/// the holder's to merge: `failed` gives it up, `delivered` is refused.
fn report(authority: &mut Authority, command: &Value, binding: &Binding<'_>) -> Result<Value, Refused> {
  let slot = pending_slot(authority, binding.index);
  let Some(open) = slot.as_ref().filter(|open| open.binds(binding.commits)) else {
    return Err((409, "transition_denied"));
  };
  let delivered = command["outcome"]["kind"] == "delivered";
  if delivered && !open.merging {
    return Err((403, "delivery_not_authorized"));
  }
  let operation = match &open.authorized {
    Authorized::Admin(operation) => json!(operation),
    Authorized::AwaitingAdmin | Authorized::Automatic => Value::Null,
  };
  *slot = None;
  if delivered {
    lifecycle::delivered(authority, binding.index);
  }
  let state = if delivered { "delivered" } else { "failed" };
  Ok(answer(binding, (state, operation), &[]))
}

/// The branch must be the module's now and the commits differ.
fn checked(command: &Value, binding: &Binding<'_>) -> Result<(), Refused> {
  if command["branch"].as_str() != Some(binding.branch.as_str()) {
    return Err((409, "transition_denied"));
  }
  if binding.commits.0 == binding.commits.1 {
    return Err((422, "invalid"));
  }
  Ok(())
}

/// Takes the kept authorization `cited` when it is of exactly this binding: `false` when it is not.
fn take_authorization(authority: &mut Authority, cited: &str, binding: &Binding<'_>) -> bool {
  let found = authority.authorizations.iter().position(|kept| {
    kept.operation == cited
      && kept.module == binding.index
      && Some(kept.claim.as_str()) == binding.claim.as_deref()
      && kept.branch == binding.branch
      && (kept.source_commit.as_str(), kept.base_commit.as_str()) == binding.commits
  });
  found
    .map(|position| authority.authorizations.remove(position))
    .is_some()
}

/// `{kind: "delivery", delivery: ModuleDeliveryRead, effects}`.
fn answer(binding: &Binding<'_>, (state, authorization): (&str, Value), effects: &[Value]) -> Value {
  let delivery = json!({"subject": {"kind": "module", "id": module_id(binding.index)}, "claim_id": binding.claim,
    "authorization_operation_id": authorization, "branch": binding.branch, "source_commit": binding.commits.0,
    "base_commit": binding.commits.1, "state": state, "revision": 1});
  json!({"kind": "delivery", "delivery": delivery, "effects": effects})
}

fn module_claim(authority: &Authority, index: usize) -> Option<String> {
  let modules = authority.modules.as_ref()?;
  modules.get(index)?.claim.map(|claim| claim.id(module_number(index)))
}

/// The module's branch as an Admin left it.
fn branch_now(authority: &Authority, index: usize) -> String {
  authority.branches.get(&index).map_or_else(
    || {
      let modules = authority.modules.as_deref().unwrap_or_default();
      modules.get(index).map_or("", |module| module.branch).to_owned()
    },
    |changed| changed.branch.clone(),
  )
}

fn pending_slot(authority: &mut Authority, index: usize) -> &mut Option<PendingDelivery> {
  let modules = authority
    .modules
    .as_mut()
    .expect("a delivery's module is one of those set");
  &mut modules[index].pending
}

fn pending_mut(authority: &mut Authority, index: usize) -> Option<&mut PendingDelivery> {
  pending_slot(authority, index).as_mut()
}
