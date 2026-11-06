//! The S10 list reads (`orchester_api::v2`): the task list's filters, search, sorts and board counts, module summaries
//! counted by core's rules, the resource library and pickers, and the workspace's metadata.

use chrono::NaiveDate;
use orchester_api::{
  SharedResourceKind,
  v2::{
    BurndownDayRead, CallerRole, ForecastRead, ModuleBurndownRead, ModuleListRequest, ModuleProgressRead, OutlookRead,
    ResourceChoice, ResourceLibraryRequest, ResourcePickerRequest, SearchText, SearchTextError, TaskBoardCounts,
    TaskFilters, TaskListRequest, TaskSort, WorkspaceMetadata,
  },
};
use orchester_core::{
  forecast::{Forecast, History, Outlook},
  id::{ModuleId, ReleaseId, ResourceId, TagId},
  module::{
    ModuleState,
    burndown::{Burndown, BurndownDay},
    measure::WorkUnit,
    progress::Progress,
  },
  resource::VersionNumber,
  task::organization::{Priority, WorkflowStatus},
};
use serde_json::{Value, json};

fn day(day: u32) -> NaiveDate {
  NaiveDate::from_ymd_opt(2026, 11, day).expect("a November day")
}

#[test]
fn authority_lists_task_list_request_defaults_and_filters() {
  let request: TaskListRequest = serde_json::from_value(json!({})).expect("everything defaults");
  assert_eq!(request, TaskListRequest::default());
  assert_eq!(request.sort, TaskSort::Created);
  assert_eq!(request.page.limit(), 50);
  let (tag, module, release) = (TagId::generate(), ModuleId::generate(), ReleaseId::generate());
  let wire = json!({
    "filters":{
      "statuses":["in_review","canceled"],"priorities":["urgent","none"],"tags":[tag],"all_tags":true,
      "pipelines":["default",null],"modules":[module,null],"releases":[release],"search":"sign in"
    },
    "sort":"priority",
    "page":{"after":null,"limit":20}
  });
  let request: TaskListRequest = serde_json::from_value(wire.clone()).expect("a request");
  assert_eq!(
    request.filters,
    TaskFilters {
      statuses: vec![WorkflowStatus::InReview, WorkflowStatus::Canceled],
      priorities: vec![Priority::Urgent, Priority::None],
      tags: vec![tag],
      all_tags: true,
      pipelines: vec![Some("default".to_owned()), None],
      modules: vec![Some(module), None],
      releases: vec![Some(release)],
      search: Some(SearchText::new("sign in").expect("search")),
    }
  );
  assert_eq!(serde_json::to_value(&request).expect("serialize"), wire);
  for sort in ["created", "newest", "title", "status", "priority"] {
    assert!(serde_json::from_value::<TaskSort>(json!(sort)).is_ok());
  }
  for invalid in [
    json!({"filters":{"assignee":null}}),
    json!({"sort":"manual"}),
    json!({"filters":{"statuses":["InReview"]}}),
    json!({"view":"active"}),
  ] {
    assert!(serde_json::from_value::<TaskListRequest>(invalid).is_err());
  }
}

#[test]
fn authority_lists_search_is_trimmed_and_bounded() {
  assert_eq!(SearchText::new("  KON-12 ").expect("search").as_str(), "KON-12");
  assert_eq!(SearchText::new("   "), Err(SearchTextError::Empty));
  assert_eq!(SearchText::new(&"é".repeat(201)), Err(SearchTextError::TooLong));
  assert!(SearchText::new(&"é".repeat(200)).is_ok());
  assert!(serde_json::from_value::<TaskFilters>(json!({"search":""})).is_err());
}

#[test]
fn authority_lists_board_counts_shape() {
  let wire = json!({"backlog":3,"todo":2,"in_progress":1,"in_review":0,"done":5,"canceled":1,"cursor":"1"});
  let counts: TaskBoardCounts = serde_json::from_value(wire.clone()).expect("counts");
  assert_eq!((counts.backlog, counts.done, counts.cursor.position().get()), (3, 5, 1));
  assert_eq!(serde_json::to_value(&counts).expect("serialize"), wire);
  let mut unknown = wire;
  unknown["needs_you"] = json!(1);
  assert!(serde_json::from_value::<TaskBoardCounts>(unknown).is_err());
}

#[test]
fn authority_lists_module_summaries_follow_core() {
  let progress = ModuleProgressRead::from_core(Progress {
    unit: WorkUnit::Tasks,
    scope: 4,
    done: 1,
  });
  assert_eq!(
    serde_json::to_value(progress).expect("progress"),
    json!({"unit":"tasks","scope":4,"done":1})
  );
  let dates = Forecast {
    remaining: 3,
    history_days: 30,
    history_delivered: 9,
    outlook: Outlook::Dates {
      likely: day(10),
      safe: day(14),
      past_horizon: false,
    },
  };
  let wire = json!({"remaining":3,"history_days":30,"history_delivered":9,
    "outlook":{"kind":"dates","likely":"2026-11-10","safe":"2026-11-14","past_horizon":false}});
  assert_eq!(
    serde_json::to_value(ForecastRead::from_core(&dates)).expect("forecast"),
    wire
  );
  let short = Forecast::of(2, &History::new(vec![1; 3]), day(3), 1);
  assert_eq!(
    ForecastRead::from_core(&short).outlook,
    OutlookRead::NotEnoughHistory {}
  );
  assert!(serde_json::from_value::<OutlookRead>(json!({"kind":"done","likely":"2026-11-10"})).is_err());

  let burndown = Burndown {
    unit: WorkUnit::Tasks,
    days: vec![BurndownDay {
      date: day(2),
      scope: None,
      remaining: None,
      ideal: Some(1.5),
      added: 0,
      removed: 0,
    }],
    start_scope: 2,
  };
  let module = ModuleId::generate();
  let cursor = orchester_api::v2::EventCursor::new("1").expect("cursor");
  let read = ModuleBurndownRead::from_core(module, (&burndown, &dates), cursor);
  let read_wire = serde_json::to_value(&read).expect("burndown");
  assert_eq!(
    read_wire["days"],
    json!([{"date":"2026-11-02","scope":null,"remaining":null,"ideal":1.5,"added":0,"removed":0}])
  );
  assert_eq!(
    serde_json::from_value::<ModuleBurndownRead>(read_wire).expect("read"),
    read
  );
  let missing: Value = json!({"date":"2026-11-02","remaining":null,"ideal":null,"added":0,"removed":0});
  assert!(serde_json::from_value::<BurndownDayRead>(missing).is_err());
}

#[test]
fn authority_lists_module_list_and_library_requests() {
  let modules: ModuleListRequest = serde_json::from_value(json!({"state":"closed"})).expect("closed modules");
  assert_eq!(modules.state, Some(ModuleState::Closed));
  assert_eq!(
    serde_json::from_value::<ModuleListRequest>(json!({})).expect("all"),
    ModuleListRequest::default()
  );
  let library: ResourceLibraryRequest =
    serde_json::from_value(json!({"kind":"agent_profile","search":"review"})).expect("library");
  assert_eq!(
    (library.kind, library.archived),
    (Some(SharedResourceKind::AgentProfile), false)
  );
  assert!(
    serde_json::from_value::<ResourcePickerRequest>(json!({})).is_err(),
    "a picker names its kind"
  );
  assert!(serde_json::from_value::<ResourcePickerRequest>(json!({"kind":"pipeline","archived":true})).is_err());
  let id = ResourceId::generate();
  let choice = ResourceChoice {
    id,
    kind: SharedResourceKind::Pipeline,
    slug: "default".to_owned(),
    name: "Default".to_owned(),
    latest: VersionNumber::FIRST,
  };
  let wire = json!({"id":id,"kind":"pipeline","slug":"default","name":"Default","latest":1});
  assert_eq!(serde_json::to_value(&choice).expect("choice"), wire);
  assert_eq!(serde_json::from_value::<ResourceChoice>(wire).expect("read"), choice);
}

fn workspace_metadata_wire() -> Value {
  let project = orchester_core::id::ProjectId::generate();
  json!({
    "project":{"id":project,"slug":"kontur","address":"https://team.example/kontur","key":"KON","name":"Kontur",
      "remote_hint":null,"role":"member","next_number":13},
    "policy":{"revision":1,"automatic_merge":false},
    "capabilities":{"project_id":project,"revision":1,"role":"member","actions":[],"unresolved":[]},
    "cursor":"1",
    "claim_staleness_seconds":1800
  })
}

#[test]
fn authority_lists_workspace_metadata_refuses_unknown_fields() {
  let wire = workspace_metadata_wire();
  let metadata: WorkspaceMetadata = serde_json::from_value(wire.clone()).expect("metadata");
  assert_eq!(metadata.project.key, "KON");
  assert_eq!(serde_json::to_value(&metadata).expect("serialize"), wire);
  let mut unknown = wire;
  unknown["members"] = json!([]);
  assert!(serde_json::from_value::<WorkspaceMetadata>(unknown).is_err());
}

#[test]
fn authority_lists_workspace_metadata_carries_the_claim_staleness_threshold() {
  let wire = workspace_metadata_wire();
  let metadata: WorkspaceMetadata = serde_json::from_value(wire.clone()).expect("metadata");
  assert_eq!(metadata.claim_staleness_seconds.seconds(), 1800);
  assert_eq!(metadata.capabilities.role, CallerRole::Member, "a member reads it");
  let mut missing = wire.clone();
  missing
    .as_object_mut()
    .expect("an object")
    .remove("claim_staleness_seconds");
  assert!(serde_json::from_value::<WorkspaceMetadata>(missing).is_err());
  for invalid in [
    json!(299),
    json!(86401),
    json!(-1),
    json!(900.5),
    json!("900"),
    json!(null),
  ] {
    let mut malformed = wire.clone();
    malformed["claim_staleness_seconds"] = invalid;
    assert!(serde_json::from_value::<WorkspaceMetadata>(malformed).is_err());
  }
}
