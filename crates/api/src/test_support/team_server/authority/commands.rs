//! The stand-in's commands, operations and policy under `/api/v2` (`docs/claim-authority.md`, `module-branches.md`,
//! `operation-receipts.md` and `workspace-policy.md` of the platform), for the QA states and tests that act on a team
//! project: `claim.acquire`, `claim.take_over` and `claim.release` of a task or a module, `module.branch.set`,
//! `work.start` and `work.report` ([`execution`]), `delivery.authorize`, `delivery.request` and `delivery.report`, each
//! kept by its operation id with its request and answered again unchanged, another request under the same id `409
//! operation_mismatch`; `GET operations/{id}`; and `GET`/`PATCH policy`. The person reading holds what they took in
//! their one session here. A command can meet a [`Plan`] instead of its rule once, as when someone else acted first.
//! Each committed command invalidates the lists, the modules and the workspace on the subscription.

use serde_json::{Value, json};

use super::{
  Authority, AuthorityClaim, Person, Standing, delivery, execution,
  listing::task_id,
  work::{BranchChange, module_id, module_number},
};

/// What the next command of a kind meets in place of its own rule, once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
  /// It is refused with this status and code, and nothing changes.
  Refuse(u16, &'static str),
  /// The task's holder renews the claim first: a take-over finds it fresh, `409 claim_conflict`.
  RenewedFirst,
  /// `by` gives the module the branch `branch` first: the change is refused `409 revision_conflict`.
  BranchFirst { by: Person, branch: &'static str },
}

/// A refusal's status and code.
pub(super) type Refused = (u16, &'static str);

/// The generations the claims acquired here are numbered from, past those the reads set and their take-overs.
const ISSUED_FROM: u8 = 0x80;

/// `POST …/commands` of `project` with `body`: the receipt or the refusal, kept by the operation's id.
pub(super) fn command(authority: &mut Authority, project: &str, body: &str) -> (u16, Value) {
  let Ok(body) = serde_json::from_str::<Value>(body) else {
    return (422, refusal(None, "invalid"));
  };
  let operation = body["operation_id"].as_str().unwrap_or_default().to_owned();
  let identity = json!({"expected": body["expected"], "command": body["command"]});
  if let Some((_, known, status, answer)) = authority.operations.iter().find(|(id, ..)| *id == operation) {
    return if *known == identity {
      (if *status == 201 { 200 } else { *status }, answer.clone())
    } else {
      (409, refusal(Some(&operation), "operation_mismatch"))
    };
  }
  let (status, answer) = match carry_out(authority, (project, &operation), &body["command"]) {
    Ok(result) => {
      authority.commit(vec![
        json!({"kind": "task_list"}),
        json!({"kind": "module_list"}),
        json!({"kind": "workspace"}),
      ]);
      let cursor = authority.cursor().to_string();
      (
        201,
        json!({"operation_id": operation, "state": "committed", "result": result, "cursor": cursor}),
      )
    }
    Err((status, code)) => (status, refusal(Some(&operation), code)),
  };
  authority.operations.push((operation, identity, status, answer.clone()));
  (status, answer)
}

/// `GET …/operations/{id}`: the kept receipt or refusal, or `404 not_found`.
pub(super) fn operation(authority: &Authority, id: &str) -> (u16, Value) {
  match authority.operations.iter().find(|(known, ..)| known == id) {
    Some((_, _, 201, receipt)) => (
      200,
      json!({"operation_id": id, "state": "committed", "response": receipt}),
    ),
    Some((_, _, _, error)) => (200, json!({"operation_id": id, "state": "rejected", "error": error})),
    None => (404, refusal(Some(id), "not_found")),
  }
}

/// `GET …/policy`: the policy and what the caller may do with it.
pub(super) fn policy(authority: &Authority) -> Value {
  let admin = authority.standing != Standing::Member;
  json!({"policy": policy_wire(authority), "capabilities": {"read_policy": true, "change_policy": admin}})
}

/// `PATCH …/policy {revision, automatic_merge}`: a member's is `403`, a stale revision `409` with the policy now.
pub(super) fn change_policy(authority: &mut Authority, body: &str) -> (u16, Value) {
  if authority.standing == Standing::Member {
    return (403, json!({"error": "not_admin", "message": "refused"}));
  }
  let Ok(body) = serde_json::from_str::<Value>(body) else {
    return (422, json!({"error": "invalid", "message": "refused"}));
  };
  if let Some(first) = authority.policy_first.take() {
    authority.policy = (authority.policy.0 + 1, first);
  }
  if body["revision"].as_u64() != Some(authority.policy.0) {
    return (
      409,
      json!({"error": "policy_revision_conflict", "current": policy_wire(authority)}),
    );
  }
  authority.policy = (
    authority.policy.0 + 1,
    body["automatic_merge"].as_bool().unwrap_or_default(),
  );
  authority.commit(vec![json!({"kind": "workspace"})]);
  (200, policy(authority))
}

/// `{revision, automatic_merge}`.
pub(super) fn policy_wire(authority: &Authority) -> Value {
  json!({"revision": authority.policy.0, "automatic_merge": authority.policy.1})
}

fn carry_out(authority: &mut Authority, (project, operation): (&str, &str), command: &Value) -> Result<Value, Refused> {
  let kind = command["kind"].as_str().unwrap_or_default();
  if let Some(position) = authority.planned.iter().position(|(planned, _)| *planned == kind) {
    let (_, plan) = authority.planned.remove(position);
    planned(authority, command, plan)?;
  }
  match kind {
    "claim.acquire" | "claim.take_over" => take(authority, kind, command),
    "claim.release" => release(authority, command),
    "module.branch.set" => branch(authority, project, command),
    "work.start" => execution::start(authority, command),
    "work.report" => execution::report(authority, command),
    "delivery.authorize" | "delivery.request" | "delivery.report" => {
      delivery::deliver(authority, (operation, kind), command)
    }
    _ => Err((422, "invalid")),
  }
}

/// What `plan` makes of the command before its rule: a refusal, with what someone else did first.
fn planned(authority: &mut Authority, command: &Value, plan: Plan) -> Result<(), Refused> {
  match plan {
    Plan::Refuse(status, code) => Err((status, code)),
    Plan::RenewedFirst => {
      if let Some(Some(claim)) = subject_claim(authority, &command["subject"]) {
        claim.stale = false;
        claim.renewed_minutes_ago = 0;
      }
      Err((409, "claim_conflict"))
    }
    Plan::BranchFirst { by, branch } => {
      let index = module_index(authority, &command["module_id"]).ok_or((404, "not_found"))?;
      let revision = authority.branches.get(&index).map_or(1, |changed| changed.revision) + 1;
      authority.branches.insert(
        index,
        BranchChange {
          branch: branch.to_owned(),
          revision,
          by,
        },
      );
      Err((409, "revision_conflict"))
    }
  }
}

/// `claim.acquire` or `claim.take_over` of a task or a module, by the person. The person's own claim of a task is
/// answered unchanged; another task under someone else's module claim is theirs to claim. Each claim acquired anew has
/// an id of its own, as the server issues one, so it is never one released before.
fn take(authority: &mut Authority, kind: &str, command: &Value) -> Result<Value, Refused> {
  let subject = command["subject"].clone();
  let own =
    subject_claim(authority, &subject).is_some_and(|claim| claim.is_some_and(|held| held.holder == Person::You));
  if subject["kind"] == "task" && !own && module_holder(authority, &subject).is_some_and(|holder| holder != Person::You)
  {
    return Err((409, "module_claim_conflict"));
  }
  let number = subject_number(authority, &subject).ok_or((404, "not_found"))?;
  authority.issued = authority.issued.wrapping_add(1);
  let fresh = AuthorityClaim {
    generation: ISSUED_FROM.wrapping_add(authority.issued),
    ..AuthorityClaim::taken(Person::You, None)
  };
  let claim = subject_claim(authority, &subject).ok_or((404, "not_found"))?;
  let taken = match (kind, *claim) {
    ("claim.acquire", Some(held)) if held.holder == Person::You => held,
    ("claim.acquire", Some(_)) => return Err((409, "claim_conflict")),
    ("claim.acquire", None) => fresh,
    (_, Some(held)) if held.stale && command["observed_claim_id"] == held.id(number) => {
      AuthorityClaim::taken(Person::You, Some(&held))
    }
    _ => return Err((409, "claim_conflict")),
  };
  *claim = Some(taken);
  Ok(
    json!({"kind": "claim", "subject": subject, "claim": taken.wire(&subject, number), "children": [],
    "effects": []}),
  )
}

/// `claim.release` of the claim the authority names, the person's own.
fn release(authority: &mut Authority, command: &Value) -> Result<Value, Refused> {
  let subject = command["authority"]["subject"].clone();
  let number = subject_number(authority, &subject).ok_or((404, "not_found"))?;
  let claim = subject_claim(authority, &subject).ok_or((404, "not_found"))?;
  let released = match *claim {
    Some(held) if held.holder == Person::You && command["authority"]["claim_id"] == held.id(number) => held,
    _ => return Err((409, "claim_lost")),
  };
  *claim = None;
  Ok(json!({"kind": "released", "subject": subject, "released_claim_id": released.id(number), "effects": []}))
}

/// `module.branch.set` by an Admin: the module of `project` with its new branch.
fn branch(authority: &mut Authority, project: &str, command: &Value) -> Result<Value, Refused> {
  if authority.standing == Standing::Member {
    return Err((403, "forbidden"));
  }
  let index = module_index(authority, &command["module_id"]).ok_or((404, "not_found"))?;
  let branch = command["branch"].as_str().ok_or((422, "invalid"))?.to_owned();
  let revision = authority.branches.get(&index).map_or(1, |changed| changed.revision) + 1;
  authority.branches.insert(
    index,
    BranchChange {
      branch,
      revision,
      by: Person::You,
    },
  );
  let module = module_read(authority, project, index);
  Ok(json!({"kind": "module", "module": module, "effects": []}))
}

/// The module at `index` of `project` as a `ModuleRead`, with its branch as an Admin left it.
pub(super) fn module_read(authority: &Authority, project: &str, index: usize) -> Value {
  let modules = authority.modules.as_deref().unwrap_or_default();
  let module = &modules[index];
  let capabilities = module_capabilities(authority, project, index);
  module.read(index, &capabilities, authority.branches.get(&index))
}

/// The caller's capabilities on task `id` of `project`, as `docs/workspace-reads.md` lists them by its effective claim:
/// its own `claim`, or the one `module_held` by its module's holder.
pub(super) fn task_capabilities(
  authority: &Authority,
  (project, id): (&str, &str),
  (claim, module_held): (Option<&AuthorityClaim>, Option<Person>),
) -> Value {
  let subject = json!({"kind": "task", "id": id});
  let (acquire, take_over, release) = match (claim, module_held) {
    (_, Some(holder)) => {
      let acquire = (holder != Person::You).then_some("module_required");
      (acquire, Some("module_required"), Some("module_required"))
    }
    (Some(claim), None) => claim_rights(claim),
    (None, None) => (None, Some("claim_required"), Some("claim_required")),
  };
  capabilities(authority, project, &subject, [acquire, take_over, release], false)
}

/// The caller's capabilities on the module at `index` of `project`.
fn module_capabilities(authority: &Authority, project: &str, index: usize) -> Value {
  let subject = json!({"kind": "module", "id": module_id(index)});
  let claim = authority.modules.as_ref().and_then(|modules| modules.get(index)?.claim);
  let rights = claim
    .as_ref()
    .map_or((None, Some("claim_required"), Some("claim_required")), claim_rights);
  capabilities(authority, project, &subject, [rights.0, rights.1, rights.2], true)
}

/// Whether the person may acquire, take over and release a subject under `claim`: each `None` when allowed, or why not.
const fn claim_rights(claim: &AuthorityClaim) -> (Option<&'static str>, Option<&'static str>, Option<&'static str>) {
  let yours = matches!(claim.holder, Person::You);
  (
    if yours { None } else { Some("claim_required") },
    if claim.stale && !yours {
      None
    } else {
      Some("claim_required")
    },
    if yours { None } else { Some("claim_required") },
  )
}

/// The workspace's capabilities with the claim commands on `subject` and an Admin's actions on it.
fn capabilities(
  authority: &Authority,
  project: &str,
  subject: &Value,
  [acquire, take_over, release]: [Option<&'static str>; 3],
  module: bool,
) -> Value {
  let action = |action: &str, reason: Option<&str>| json!({"action": action, "subject": subject, "allowed": reason.is_none(), "reason": reason});
  let admin = (authority.standing == Standing::Member).then_some("role_required");
  let mut listed = super::capabilities(project, authority.standing);
  let actions = listed["actions"]
    .as_array_mut()
    .expect("capabilities list their actions");
  actions.extend([
    action("claim.acquire", acquire),
    action("claim.take_over", take_over),
    action("claim.release", release),
    action("assignment.set", admin),
  ]);
  if module {
    actions.push(action("module.branch.set", admin));
    actions.push(action("delivery.authorize", admin));
  }
  listed
}

/// The claim of `subject`, a task or a module; `None` when there is no such subject.
fn subject_claim<'a>(authority: &'a mut Authority, subject: &Value) -> Option<&'a mut Option<AuthorityClaim>> {
  if subject["kind"] == "module" {
    let index = module_index(authority, &subject["id"])?;
    return authority
      .modules
      .as_mut()?
      .get_mut(index)
      .map(|module| &mut module.claim);
  }
  let tasks = authority.tasks.as_mut()?;
  let task = tasks.iter_mut().find(|task| subject["id"] == task_id(task))?;
  Some(&mut task.claim)
}

/// The number the claims of `subject` are made from.
fn subject_number(authority: &Authority, subject: &Value) -> Option<u32> {
  if subject["kind"] == "module" {
    return module_index(authority, &subject["id"]).map(module_number);
  }
  let tasks = authority.tasks.as_ref()?;
  tasks
    .iter()
    .find(|task| subject["id"] == task_id(task))
    .map(|task| task.number)
}

/// Who holds the module claim that covers `subject`: the module's, or the module the task is in.
pub(super) fn module_holder(authority: &Authority, subject: &Value) -> Option<Person> {
  let modules = authority.modules.as_ref()?;
  let index = if subject["kind"] == "module" {
    module_index(authority, &subject["id"])?
  } else {
    let tasks = authority.tasks.as_ref()?;
    tasks.iter().find(|task| subject["id"] == task_id(task))?.module?
  };
  modules.get(index)?.claim.map(|claim| claim.holder)
}

pub(super) fn module_index(authority: &Authority, id: &Value) -> Option<usize> {
  let count = authority.modules.as_ref()?.len();
  (0..count).find(|index| *id == module_id(*index))
}

/// `{operation_id?, code, details}`: the team-authority refusal.
fn refusal(operation: Option<&str>, code: &str) -> Value {
  match operation {
    Some(operation) => json!({"operation_id": operation, "code": code, "details": {}}),
    None => json!({"code": code, "details": {}}),
  }
}
