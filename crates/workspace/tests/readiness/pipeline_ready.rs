//! What keeps a pipeline of the hub from running as it is, as the page's *Ready?* lists it (`Pipelines · v2` P1, P8):
//! the steps that cannot run, then what the readiness read says, then why it cannot be told.

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId},
  resource::{Pipeline, Slug},
  sharing::Missing,
  store::ResourceStore,
};
use orchester_workspace::{
  pipelines::{
    flow::Runs,
    hub::HubPipeline,
    ready::{Problem, problems, step_problems},
  },
  readiness::Undecided,
};

use crate::support::{Fixture, at, name, pipeline, profile_on};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

fn listed(fixture: &Fixture, id: ResourceId) -> HubPipeline {
  fixture
    .workspace
    .pipeline_hub(&fixture.root, &fixture.vault)
    .expect("the hub")
    .pipelines()
    .iter()
    .find(|found| found.id == id)
    .expect("listed")
    .clone()
}

fn draft(fixture: &Fixture, slug: &str, body: &Pipeline) -> ResourceId {
  fixture
    .db()
    .create_resource(Slug::new(slug).expect("slug"), name(slug), body, at(10))
    .expect("a draft")
    .id()
}

fn found(pipeline: &HubPipeline) -> Vec<Problem> {
  problems(&pipeline.flow, pipeline.standing.as_ref())
}

#[test]
fn readiness_pipeline_ready_a_pipeline_that_can_run_lists_nothing() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let id = fixture.own("delivery", &pipeline(&[Some(builder), Some(builder)], None));

  let read = listed(&fixture, id);
  assert!(found(&read).is_empty());
  assert!(step_problems(&read.flow).is_empty());
}

#[test]
fn readiness_pipeline_ready_a_draft_names_each_step_that_has_nothing_to_run() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let id = draft(&fixture, "unfinished", &pipeline(&[Some(builder), None, None], None));

  let read = listed(&fixture, id);
  assert_eq!(
    read.standing, None,
    "a pipeline never published has no readiness to read"
  );
  let steps = step_problems(&read.flow);
  assert_eq!(steps.len(), 2);
  assert_eq!(
    steps.iter().map(|step| step.number).collect::<Vec<_>>(),
    [2, 3],
    "in the order of the flow, by the number the card carries"
  );
  assert_eq!(&*steps[0].name, "Step 1");
  assert!(steps.iter().all(|step| step.runs == Runs::NoProfile));
  assert_eq!(
    found(&read),
    steps.into_iter().map(Problem::Step).collect::<Vec<_>>(),
    "the draft's problems are its steps"
  );
}

#[test]
fn readiness_pipeline_ready_a_profile_that_lost_its_connection_is_undecided_and_named() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let lost = fixture.own("lost", &profile_on(ConnectionId::generate(), &[]));
  let id = fixture.own("stuck", &pipeline(&[Some(builder), Some(lost)], None));

  let read = listed(&fixture, id);
  assert!(
    step_problems(&read.flow).is_empty(),
    "every step names a published profile"
  );
  match found(&read).as_slice() {
    [Problem::Undecided(Undecided::Member { id, why })] => {
      assert_eq!(*id, lost, "the profile that cannot be told");
      assert!(matches!(**why, Undecided::ConnectionGone(_)));
    }
    other => panic!("one undecided profile expected, got {other:?}"),
  }
}

#[test]
fn readiness_pipeline_ready_a_step_that_names_a_missing_profile_is_listed_once() {
  let fixture = Fixture::local();
  let unknown = ResourceId::generate();
  let id = fixture.own("stuck", &pipeline(&[Some(unknown)], None));

  let read = listed(&fixture, id);
  assert_eq!(
    found(&read),
    [Problem::Step(step_problems(&read.flow).remove(0))],
    "the readiness's own *cannot tell* is the same step, so it is left out"
  );
  assert_eq!(
    step_problems(&read.flow)[0].runs,
    Runs::ProfileNotPublished { id: unknown }
  );
}

#[test]
fn readiness_pipeline_ready_an_mcp_program_this_computer_lacks_is_a_missing_part() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let server = fixture.local_server("tracker", "orchester-no-such-program-for-the-test");
  let builder = fixture.own("builder", &profile_on(connection, &[server]));
  let id = fixture.own("delivery", &pipeline(&[Some(builder)], None));

  let read = listed(&fixture, id);
  assert!(step_problems(&read.flow).is_empty());
  let all = found(&read);
  assert_eq!(all.len(), 1, "one part: {all:?}");
  assert!(
    matches!(&all[0], Problem::Missing(Missing::LocalMcpProgram { .. })),
    "the program of the server its profile chooses: {all:?}"
  );
}
