//! The S10 read contract's shapes (`orchester_api::v2`): page requests and their bounds, pages, capabilities, claims,
//! work, and task and module reads, each refusing unknown fields and missing nullable ones.

use orchester_api::{
  Attribution, ModuleFields, SharedModule, SharedTask, TaskContent, UserRef,
  v2::{
    Action, ActionCapability, BundleRef, CallerRole, Capabilities, ClaimRead, ClosedHow, CommandKind,
    DEFAULT_PAGE_LIMIT, DecisionId, DenialReason, EventCursor, ExecutionState, MAX_PAGE_LIMIT, ModuleRead, Page,
    PageCursor, PageRequest, PageRequestError, PendingAuthorization, PendingDeliveryRead, Subject, TaskClosure,
    TaskRead, WorkRead,
  },
};
use orchester_core::{
  blob::ContentHash,
  id::{ClaimId, ModuleId, ProjectId, ReleaseId, RunId, TaskId, UserId, WorkId},
  module::{Horizon, ModuleState},
  run::CommitId,
  sharing::TaskRevision,
  task::{
    TaskStatus,
    organization::{Priority, WorkflowStatus},
  },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(value: &T, wire: &Value) {
  assert_eq!(&serde_json::to_value(value).expect("serialize"), wire);
  assert_eq!(&serde_json::from_value::<T>(wire.clone()).expect("read"), value);
}

/// `wire` without `field`, and with an extra one: both refused.
fn refuses_missing_and_unknown<T: for<'de> Deserialize<'de>>(wire: &Value, field: &str) {
  let mut missing = wire.clone();
  missing.as_object_mut().expect("an object").remove(field);
  assert!(serde_json::from_value::<T>(missing).is_err(), "{field} is required");
  let mut unknown = wire.clone();
  unknown
    .as_object_mut()
    .expect("an object")
    .insert("extra".to_owned(), json!(1));
  assert!(
    serde_json::from_value::<T>(unknown).is_err(),
    "unknown fields are refused"
  );
}

fn cursor() -> EventCursor {
  EventCursor::new("1").expect("cursor")
}

fn user() -> UserRef {
  UserRef {
    id: UserId::generate(),
    display_name: "Ada".to_owned(),
  }
}

fn capabilities(project_id: ProjectId) -> (Capabilities, Value) {
  let kind = CommandKind::new("claim.acquire".to_owned()).expect("kind");
  let capabilities = Capabilities {
    project_id,
    revision: TaskRevision::FIRST,
    role: CallerRole::Member,
    actions: vec![
      ActionCapability::allowed(Action::Read, None),
      ActionCapability::denied(Action::Command(kind), None, DenialReason::DecisionRequired),
    ],
    unresolved: vec![DecisionId::D3],
  };
  let wire = json!({
    "project_id":project_id,"revision":1,"role":"member",
    "actions":[
      {"action":"read","subject":null,"allowed":true,"reason":null},
      {"action":"claim.acquire","subject":null,"allowed":false,"reason":"decision_required"}
    ],
    "unresolved":["D3"]
  });
  (capabilities, wire)
}

#[test]
fn authority_reads_page_request_bounds() {
  let first: PageRequest = serde_json::from_value(json!({})).expect("defaults");
  assert_eq!((first.after(), first.limit()), (None, DEFAULT_PAGE_LIMIT));
  assert_eq!(first, PageRequest::first());
  assert_eq!((DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT), (50, 200));
  let after = PageCursor::new("p2".to_owned()).expect("cursor");
  let largest = PageRequest::new(Some(after), Some(200)).expect("the maximum");
  roundtrip(&largest, &json!({"after":"p2","limit":200}));
  roundtrip(&PageRequest::first(), &json!({"after":null,"limit":50}));
  for limit in [0, 201, u16::MAX] {
    assert_eq!(
      PageRequest::new(None, Some(limit)),
      Err(PageRequestError::LimitOutOfRange(limit))
    );
    assert!(serde_json::from_value::<PageRequest>(json!({"limit":limit})).is_err());
  }
  assert!(serde_json::from_value::<PageRequest>(json!({"limit":10,"offset":5})).is_err());
  assert!(serde_json::from_value::<PageRequest>(json!({"after":""})).is_err());
}

#[test]
fn authority_reads_page_shape() {
  let next = PageCursor::new("p2".to_owned()).expect("cursor");
  let page = Page {
    items: vec![1_u32, 2],
    next: Some(next),
    cursor: cursor(),
  };
  let wire = json!({"items":[1,2],"next":"p2","cursor":"1"});
  roundtrip(&page, &wire);
  let last = Page::<u32> {
    items: vec![],
    next: None,
    cursor: cursor(),
  };
  roundtrip(&last, &json!({"items":[],"next":null,"cursor":"1"}));
  refuses_missing_and_unknown::<Page<u32>>(&wire, "next");
  refuses_missing_and_unknown::<Page<u32>>(&wire, "cursor");
}

#[test]
fn authority_reads_capabilities_shape() {
  let project_id = ProjectId::generate();
  let (capabilities, wire) = capabilities(project_id);
  roundtrip(&capabilities, &wire);
  refuses_missing_and_unknown::<Capabilities>(&wire, "unresolved");
  let task = TaskId::generate();
  let on_task = ActionCapability::denied(
    Action::ExportShared,
    Some(Subject::Task { id: task }),
    DenialReason::RoleRequired,
  );
  let action =
    json!({"action":"export.shared","subject":{"kind":"task","id":task},"allowed":false,"reason":"role_required"});
  roundtrip(&on_task, &action);
  refuses_missing_and_unknown::<ActionCapability>(&action, "subject");
  refuses_missing_and_unknown::<ActionCapability>(&action, "reason");
  for contradiction in [
    json!({"action":"read","subject":null,"allowed":true,"reason":"claim_lost"}),
    json!({"action":"read","subject":null,"allowed":false,"reason":null}),
  ] {
    assert!(serde_json::from_value::<ActionCapability>(contradiction).is_err());
  }
  for role in ["member", "admin", "server_admin"] {
    assert!(serde_json::from_value::<CallerRole>(json!(role)).is_ok());
  }
  for reason in [
    "role_required",
    "claim_required",
    "claim_lost",
    "module_required",
    "decision_required",
    "configuration_invalid",
  ] {
    assert!(serde_json::from_value::<DenialReason>(json!(reason)).is_ok());
  }
}

#[test]
fn authority_reads_actions_are_command_kinds_or_their_own() {
  assert_eq!(
    serde_json::from_value::<Action>(json!("server.admin")).expect("action"),
    Action::ServerAdmin
  );
  let kind = serde_json::from_value::<Action>(json!("work.start_run")).expect("a command kind");
  assert_eq!(kind.as_str(), "work.start_run");
  for invalid in [
    "",
    "Claim.acquire",
    "claim..acquire",
    "claim.",
    "claim acquire",
    "claim-acquire",
  ] {
    assert!(serde_json::from_value::<Action>(json!(invalid)).is_err(), "{invalid:?}");
  }
  for reserved in ["read", "export.shared", "server.admin"] {
    assert!(CommandKind::new(reserved.to_owned()).is_err());
  }
}

fn claim(subject: Subject) -> (ClaimRead, Value) {
  let (id, holder, parent) = (ClaimId::generate(), user(), ClaimId::generate());
  let claim = ClaimRead {
    subject,
    id,
    holder: holder.clone(),
    acquired_at: 1_000,
    last_execution_heartbeat_at: 2_000,
    stale_at: 902_000,
    stale: false,
    parent_module_claim_id: Some(parent),
  };
  let wire = json!({
    "subject":serde_json::to_value(subject).expect("subject"),"id":id,
    "holder":{"id":holder.id,"display_name":"Ada"},
    "acquired_at":1_000,"last_execution_heartbeat_at":2_000,"stale_at":902_000,"stale":false,
    "parent_module_claim_id":parent
  });
  (claim, wire)
}

fn work(subject: Subject, claim_id: ClaimId) -> (WorkRead, Value) {
  let (id, run) = (WorkId::generate(), RunId::generate());
  let sha256 = ContentHash::new(&"b".repeat(64)).expect("hash");
  let work = WorkRead {
    id,
    subject,
    claim_id,
    revision: TaskRevision::FIRST,
    state: ExecutionState::AwaitingHuman,
    runs: vec![run],
    bundle: BundleRef {
      sha256: sha256.clone(),
      schema_version: 1,
    },
    created_at: 3_000,
    updated_at: 4_000,
  };
  let wire = json!({
    "id":id,"subject":serde_json::to_value(subject).expect("subject"),"claim_id":claim_id,"revision":1,
    "state":"awaiting_human","runs":[run],"bundle":{"sha256":sha256,"schema_version":1},
    "created_at":3_000,"updated_at":4_000
  });
  (work, wire)
}

#[test]
fn authority_reads_claim_and_work_shapes() {
  let subject = Subject::Module {
    id: ModuleId::generate(),
  };
  let (claim, wire) = claim(subject);
  roundtrip(&claim, &wire);
  refuses_missing_and_unknown::<ClaimRead>(&wire, "parent_module_claim_id");
  let mut without_parent = wire;
  without_parent["parent_module_claim_id"] = Value::Null;
  assert_eq!(
    serde_json::from_value::<ClaimRead>(without_parent)
      .expect("null")
      .parent_module_claim_id,
    None
  );
  let (work, wire) = work(subject, claim.id);
  roundtrip(&work, &wire);
  refuses_missing_and_unknown::<WorkRead>(&wire, "runs");
  for state in [
    "running",
    "paused",
    "awaiting_human",
    "succeeded",
    "failed",
    "cancelled",
  ] {
    assert!(serde_json::from_value::<ExecutionState>(json!(state)).is_ok());
  }
  assert!(serde_json::from_value::<ExecutionState>(json!("waiting_for_answer")).is_err());
}

#[test]
fn authority_reads_task_read_shape() {
  let (id, by, release) = (TaskId::generate(), user(), ReleaseId::generate());
  let project_id = ProjectId::generate();
  let (capabilities, capabilities_wire) = capabilities(project_id);
  let (claim, claim_wire) = claim(Subject::Task { id });
  let (work, work_wire) = work(Subject::Task { id }, claim.id);
  let task = SharedTask {
    id,
    number: 12,
    key: "KON-12".to_owned(),
    former_key: None,
    revision: TaskRevision::FIRST,
    content: TaskContent {
      title: "Sign in".to_owned(),
      brief: String::new(),
      criteria: vec!["works".to_owned()],
      pipeline: Some("default".to_owned()),
      limits: None,
      workflow: WorkflowStatus::InProgress,
      priority: Priority::High,
      tags: vec![],
      blocking: vec![],
      blocked_reason: None,
      module: None,
      release: Some(release),
    },
    status: TaskStatus::Queued,
    has_run: true,
    created: Attribution { by: by.clone(), at: 1 },
    updated: Attribution { by: by.clone(), at: 2 },
    claim: None,
    run: None,
  };
  let read = TaskRead {
    task,
    assignee: Some(by.clone()),
    work: Some(work),
    effective_claim: Some(claim),
    capabilities,
    closed: None,
    blocked: None,
  };
  let attribution = |at| json!({"by":{"id":by.id,"display_name":"Ada"},"at":at});
  let wire = json!({
    "id":id,"number":12,"key":"KON-12","former_key":null,"revision":1,
    "title":"Sign in","brief":"","criteria":["works"],"pipeline":"default","limits":null,
    "workflow":"in_progress","priority":"high","tags":[],"blocking":[],"blocked_reason":null,"module":null,
    "release":release,
    "status":"queued","has_run":true,"created":attribution(1),"updated":attribution(2),"claim":null,"run":null,
    "assignee":{"id":by.id,"display_name":"Ada"},"work":work_wire,"effective_claim":claim_wire,
    "capabilities":capabilities_wire
  });
  roundtrip(&read, &wire);
  for field in [
    "former_key",
    "module",
    "release",
    "assignee",
    "work",
    "effective_claim",
    "capabilities",
    "brief",
  ] {
    refuses_missing_and_unknown::<TaskRead>(&wire, field);
  }
  closure_and_block_records(read, wire);
}

/// `closed` and `blocked` are sent when the server has a record and left out when it has none; an older server leaves
/// them out, and a `null` reads the same.
fn closure_and_block_records(mut read: TaskRead, mut wire: Value) {
  let (by, work) = (user(), WorkId::generate());
  let module = ModuleId::generate();
  for (how, how_wire) in [
    (
      ClosedHow::Person { by: by.clone() },
      json!({"kind":"person","by":{"id":by.id,"display_name":"Ada"}}),
    ),
    (
      ClosedHow::Delivery {
        module,
        module_name: "Checkout".to_owned(),
      },
      json!({"kind":"delivery","module":module,"module_name":"Checkout"}),
    ),
    (ClosedHow::Run { work }, json!({"kind":"run","work":work})),
  ] {
    read.closed = Some(TaskClosure { how, at: 7 });
    wire["closed"] = json!({"how": how_wire, "at": 7});
    roundtrip(&read, &wire);
  }
  read.blocked = Some(Attribution { by: by.clone(), at: 9 });
  wire["blocked"] = json!({"by":{"id":by.id,"display_name":"Ada"},"at":9});
  roundtrip(&read, &wire);
  wire["closed"] = Value::Null;
  wire["blocked"] = Value::Null;
  let unknown: TaskRead = serde_json::from_value(wire.clone()).expect("null reads as no record");
  assert_eq!((unknown.closed, unknown.blocked), (None, None));
  wire["closed"] = json!({"how": {"kind":"magic"}, "at": 7});
  assert!(serde_json::from_value::<TaskRead>(wire).is_err());
}

#[test]
fn authority_reads_module_read_shape() {
  let (id, by) = (ModuleId::generate(), user());
  let (capabilities, capabilities_wire) = capabilities(ProjectId::generate());
  let module = SharedModule {
    id,
    fields: ModuleFields {
      name: "Checkout".to_owned(),
      description: String::new(),
      start: Some("2026-11-01".to_owned()),
      target: None,
      state: ModuleState::Open,
      horizon: Horizon::Next,
      place: 2,
    },
    revision: TaskRevision::FIRST,
    created: Attribution { by: by.clone(), at: 1 },
    updated: Attribution { by: by.clone(), at: 1 },
  };
  let read = ModuleRead {
    module,
    branch: "modules/checkout".to_owned(),
    assignee: None,
    work: None,
    effective_claim: None,
    capabilities,
    pending_delivery: None,
  };
  let attribution = json!({"by":{"id":by.id,"display_name":"Ada"},"at":1});
  let wire = json!({
    "id":id,"name":"Checkout","description":"","start":"2026-11-01","target":null,"state":"open",
    "horizon":"next","place":2,"revision":1,
    "created":attribution,"updated":attribution,"branch":"modules/checkout","assignee":null,"work":null,
    "effective_claim":null,"capabilities":capabilities_wire,"pending_delivery":null
  });
  roundtrip(&read, &wire);
  for field in [
    "target",
    "horizon",
    "place",
    "branch",
    "assignee",
    "work",
    "effective_claim",
    "pending_delivery",
  ] {
    refuses_missing_and_unknown::<ModuleRead>(&wire, field);
  }

  let (source, base) = ("1".repeat(40), "2".repeat(40));
  let pending = PendingDeliveryRead::new(
    by.clone(),
    5,
    CommitId::new(&source).expect("commit"),
    CommitId::new(&base).expect("commit"),
    PendingAuthorization::AwaitingAdmin,
  )
  .expect("pending delivery");
  let read = ModuleRead {
    pending_delivery: Some(pending),
    ..read
  };
  let mut wire = wire;
  wire["pending_delivery"] = json!({
    "requested_by":{"id":by.id,"display_name":"Ada"},"requested_at":5,"source_commit":source,"base_commit":base,
    "authorization":"awaiting_admin"
  });
  roundtrip(&read, &wire);
  wire["pending_delivery"]["claim_id"] = json!(ClaimId::generate());
  assert!(serde_json::from_value::<ModuleRead>(wire).is_err());
}
