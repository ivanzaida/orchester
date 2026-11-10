//! The Pipelines hub of a local project (`Workspace::pipeline_hub`, Pipelines v2 P1): each pipeline with its steps, the
//! tasks that use it and its version; the ones that cannot run as they are, which need the person; the drafts; what was
//! archived.

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, IdempotencyKey, ResourceId},
  resource::{Pipeline, Slug},
  store::{ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_workspace::{
  pipelines::{hub::PipelineHub, library::VersionState},
  readiness::Group,
};

use crate::support::{Fixture, at, name, pipeline, profile_on};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

fn hub(fixture: &Fixture) -> PipelineHub {
  fixture
    .workspace
    .pipeline_hub(&fixture.root, &fixture.vault)
    .expect("the hub")
}

/// A pipeline that was never published, as New pipeline leaves it.
fn draft(fixture: &Fixture, slug: &str, body: &Pipeline) -> ResourceId {
  fixture
    .db()
    .create_resource(Slug::new(slug).expect("slug"), name(slug), body, at(10))
    .expect("a draft")
    .id()
}

fn use_pipeline(fixture: &Fixture, pipeline: ResourceId, tasks: usize) {
  let details = (0..tasks)
    .map(|number| TaskDetails {
      title: name(&format!("Task {number}")),
      brief: String::new(),
      acceptance_criteria: Vec::new(),
      pipeline: Some(pipeline),
      limits: TaskLimits::NONE,
    })
    .collect();
  fixture
    .db()
    .create_tasks(IdempotencyKey::generate(), details, Timestamp::now())
    .expect("tasks");
}

#[test]
fn readiness_pipeline_hub_groups_by_whether_a_pipeline_can_run_as_it_is() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let lost = fixture.own("lost", &profile_on(ConnectionId::generate(), &[]));
  let ready = fixture.own("delivery", &pipeline(&[Some(builder), Some(builder)], None));
  let stuck = fixture.own("stuck", &pipeline(&[Some(builder), Some(lost)], None));
  let unfinished = draft(&fixture, "unfinished", &pipeline(&[Some(builder), None], None));
  use_pipeline(&fixture, ready, 3);

  let read = hub(&fixture);
  assert_eq!(read.pipelines().len(), 3);
  let group = |id| read.pipelines().iter().find(|found| found.id == id).expect("listed");

  let delivery = group(ready);
  assert_eq!(delivery.group(), Group::Ready);
  assert_eq!((delivery.step_count, delivery.used_by), (2, 3));
  assert_eq!(
    delivery.state,
    VersionState::Published(delivery.number().expect("a version"))
  );
  assert!(delivery.published_at.is_some());

  let stuck = group(stuck);
  assert_eq!(stuck.group(), Group::NeedsYouHere, "what its steps run is not ready");
  assert!(!stuck.is_draft());
  assert!(
    stuck.problem.is_none(),
    "only a draft says what keeps it from publishing"
  );

  let unfinished = group(unfinished);
  assert_eq!(
    unfinished.group(),
    Group::NeedsYouHere,
    "a pipeline never published cannot run"
  );
  assert!(unfinished.is_draft());
  assert_eq!(unfinished.number(), None);
  assert_eq!(
    unfinished.problem.as_ref().map(|problem| problem.key),
    Some("pipelines.problems.noProfile"),
    "the step with no agent profile"
  );
}

#[test]
fn readiness_pipeline_hub_leaves_an_archived_pipeline_out_of_the_list_and_lists_it_apart() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let kept = fixture.own("kept", &pipeline(&[Some(builder)], None));
  let old = fixture.own("old", &pipeline(&[Some(builder)], None));
  assert_eq!(hub(&fixture).archived().len(), 0);

  fixture.db().archive_local_resource(old, at(30)).expect("archived");

  let read = hub(&fixture);
  assert_eq!(read.pipelines().len(), 1);
  assert_eq!(read.pipelines()[0].id, kept);
  let archived = read.archived();
  assert_eq!(archived.len(), 1);
  assert_eq!(archived[0].id, old);
  assert_eq!(archived[0].at, at(30));
  assert!(archived[0].latest.is_some(), "it keeps the version it had");
}

#[test]
fn readiness_pipeline_hub_carries_each_pipelines_steps_as_the_flow_they_run_in() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let published = fixture.own("delivery", &pipeline(&[Some(builder), Some(builder)], None));
  let unfinished = draft(&fixture, "unfinished", &pipeline(&[Some(builder), None], None));

  let read = hub(&fixture);
  let flow = |id| {
    let found = read.pipelines().iter().find(|found| found.id == id).expect("listed");
    found.flow.clone()
  };
  let published = flow(published);
  assert!(
    !published.draft,
    "a published pipeline without a draft shows its version"
  );
  let drawn = published.flow.as_ref().expect("two steps that wait for nothing are drawn");
  assert_eq!(drawn.stages.len(), 1, "steps that wait for nothing run side by side");
  assert_eq!(drawn.stages[0].cards.len(), 2);
  assert!(drawn.stages[0].cards.iter().all(|card| !card.runs.needs_you()));

  let unfinished = flow(unfinished);
  assert!(unfinished.draft, "a pipeline never published shows its draft");
  let drawn = unfinished.flow.as_ref().expect("drawn");
  assert!(
    drawn.stages[0].cards[1].runs.is_unassigned(),
    "its second step has no profile yet"
  );
}
