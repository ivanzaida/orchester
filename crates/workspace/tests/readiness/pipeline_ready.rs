//! What keeps a pipeline of the hub from running as it is, as the page's *Ready?* lists it (`Pipelines · v2` P1, P8):
//! the steps that cannot run, then what the readiness read says, then why it cannot be told.

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId},
  resource::{Draft as StoredDraft, FlowError, Pipeline, PipelineError, ResourceKind, Slug, StepKey},
  sharing::Missing,
  store::ResourceStore,
};
use orchester_workspace::{
  pipelines::{
    flow::Runs,
    hub::HubPipeline,
    ready::{Basis, Draft, Problem, problems, step_problems},
  },
  readiness::{Group, Undecided},
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
  problems(Basis {
    flow: &pipeline.flow,
    standing: pipeline.standing.as_ref(),
    undecided: &pipeline.undecided,
    published: pipeline.number().is_some(),
  })
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

fn lost_profile(fixture: &Fixture, slug: &str) -> ResourceId {
  fixture.own(slug, &profile_on(ConnectionId::generate(), &[]))
}

#[test]
fn readiness_pipeline_ready_a_profile_that_lost_its_connection_is_undecided_and_named() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let lost = lost_profile(&fixture, "lost");
  let id = fixture.own("stuck", &pipeline(&[Some(builder), Some(lost)], None));

  let read = listed(&fixture, id);
  assert!(
    step_problems(&read.flow).is_empty(),
    "every step names a published profile"
  );
  match found(&read).as_slice() {
    [Problem::Member(member)] => {
      assert_eq!(member.id, lost, "the profile that cannot be told");
      assert_eq!(&*member.name, "lost");
      assert_eq!(member.kind, ResourceKind::AgentProfile);
      assert!(matches!(member.why, Undecided::ConnectionGone(_)));
    }
    other => panic!("one undecided profile expected, got {other:?}"),
  }
}

#[test]
fn readiness_pipeline_ready_every_profile_that_lost_its_connection_is_named_not_only_the_first() {
  let fixture = Fixture::local();
  let first = lost_profile(&fixture, "explorer");
  let second = lost_profile(&fixture, "builder");
  let id = fixture.own(
    "stuck",
    &pipeline(&[Some(first), Some(second), Some(first)], Some(second)),
  );

  let read = listed(&fixture, id);
  let members: Vec<_> = found(&read)
    .into_iter()
    .map(|problem| match problem {
      Problem::Member(member) => member.id,
      other => panic!("only profiles expected, got {other:?}"),
    })
    .collect();
  assert_eq!(members, [first, second], "in the order of the steps, each once");
}

#[test]
fn readiness_pipeline_ready_a_step_problem_does_not_hide_a_profile_that_lost_its_connection() {
  let fixture = Fixture::local();
  let lost = lost_profile(&fixture, "lost");
  let id = fixture.own("stuck", &pipeline(&[Some(lost)], None));

  let version = fixture
    .db()
    .latest_version::<Pipeline>(id)
    .expect("read")
    .expect("published")
    .number();
  fixture
    .db()
    .save_draft(
      id,
      &StoredDraft::new(pipeline(&[Some(lost), None], None), Some(version), at(30)),
    )
    .expect("a draft");

  let read = listed(&fixture, id);
  let all = found(&read);
  assert!(
    matches!(all.as_slice(), [Problem::Step(step), Problem::Member(member)] if step.number == 2 && member.id == lost),
    "the draft's step, then the published version's profile: {all:?}"
  );
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
    "the readiness's own *cannot tell* is the same step, named another way"
  );
  assert_eq!(
    step_problems(&read.flow)[0].runs,
    Runs::ProfileNotPublished { id: unknown }
  );
}

#[test]
fn readiness_pipeline_ready_a_draft_with_no_steps_says_so() {
  let fixture = Fixture::local();
  let id = draft(&fixture, "empty", &pipeline(&[], None));

  let read = listed(&fixture, id);
  assert_eq!(found(&read), [Problem::Draft(Draft::NoSteps)]);
}

#[test]
fn readiness_pipeline_ready_a_draft_that_is_only_not_published_yet_says_so() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let id = draft(&fixture, "fresh", &pipeline(&[Some(builder), Some(builder)], None));

  let read = listed(&fixture, id);
  assert_eq!(found(&read), [Problem::Draft(Draft::NotPublished)]);
}

#[test]
fn readiness_pipeline_ready_a_draft_a_line_cannot_draw_says_so() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let mut steps = pipeline(&[Some(builder), Some(builder), Some(builder)], None).into_steps();
  // The second step waits for only the first, though the third also starts the run: a branch.
  let first = steps[0].key.clone();
  steps[1].depends_on.insert(first);
  let id = draft(&fixture, "branching", &Pipeline::new(steps).expect("a valid pipeline"));

  let read = listed(&fixture, id);
  assert_eq!(found(&read), [Problem::Draft(Draft::Branches)]);
}

#[test]
fn readiness_pipeline_ready_steps_that_wait_for_each_other_are_broken() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let id = draft(&fixture, "cycle", &pipeline(&[Some(builder)], None));
  let mut read = listed(&fixture, id);
  // A stored pipeline cannot hold a cycle (`Pipeline::new` refuses it), so the flow's failure is put in by hand.
  let key = StepKey::new("step-0").expect("key");
  let flow = std::sync::Arc::make_mut(&mut read.flow);
  flow.flow = Err(FlowError::Broken(PipelineError::Cycle(key)));

  assert_eq!(found(&read), [Problem::Draft(Draft::Broken)]);
}

#[test]
fn readiness_pipeline_ready_a_draft_over_a_published_version_lists_the_drafts_steps_only() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let id = fixture.own("delivery", &pipeline(&[Some(builder), Some(builder)], None));
  assert!(found(&listed(&fixture, id)).is_empty(), "the version tasks use can run");
  let version = fixture
    .db()
    .latest_version::<Pipeline>(id)
    .expect("read")
    .expect("published")
    .number();
  fixture
    .db()
    .save_draft(
      id,
      &StoredDraft::new(pipeline(&[Some(builder), None], None), Some(version), at(30)),
    )
    .expect("a draft");

  let read = listed(&fixture, id);
  assert_eq!(read.number(), Some(version));
  assert_eq!(read.group(), Group::Ready, "the list speaks of the version tasks use");
  assert!(read.flow.draft);
  assert!(
    matches!(found(&read).as_slice(), [Problem::Step(step)] if step.number == 2),
    "the page describes the draft: {:?}",
    found(&read)
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
