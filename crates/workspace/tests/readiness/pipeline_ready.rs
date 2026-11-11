//! What keeps a pipeline of the hub from running as it is, as the page's *Ready?* lists it (`Pipelines · v2` P1, P8):
//! the steps that cannot run, then what the readiness read says, then why it cannot be told.

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId},
  resource::{Draft as StoredDraft, FlowError, Pipeline, PipelineError, Slug, StepKey},
  sharing::Missing,
  store::ResourceStore,
};
use orchester_workspace::{
  pipelines::{
    hub::HubPipeline,
    ready::{Basis, Blocked, Draft, Problem, problems, step_problems},
  },
  readiness::{Group, ReadinessStanding, Undecided},
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
    runs: None,
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
  assert!(steps.iter().all(|step| step.blocked == Blocked::NoProfile));
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
fn readiness_pipeline_ready_a_step_whose_profile_lost_its_connection_says_so_once() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let lost = lost_profile(&fixture, "lost");
  let id = fixture.own("stuck", &pipeline(&[Some(builder), Some(lost)], None));

  let read = listed(&fixture, id);
  assert!(
    matches!(&read.undecided[..], [member] if member.id == lost && matches!(member.why, Undecided::ConnectionGone(_))),
    "the readiness cannot tell the profile: {:?}",
    read.undecided
  );
  match found(&read).as_slice() {
    [Problem::Step(step)] => {
      assert_eq!(step.number, 2, "the step that runs it, as its card stands in amber");
      assert_eq!(
        step.blocked,
        Blocked::ProfileConnectionGone {
          id: lost,
          name: "lost".into()
        }
      );
    }
    other => panic!("the step alone, not the profile again: {other:?}"),
  }
}

#[test]
fn readiness_pipeline_ready_every_step_whose_profile_lost_its_connection_is_named_and_the_integrator_once() {
  let fixture = Fixture::local();
  let first = lost_profile(&fixture, "explorer");
  let second = lost_profile(&fixture, "builder");
  let integrator = lost_profile(&fixture, "integrator");
  let id = fixture.own(
    "stuck",
    &pipeline(&[Some(first), Some(second), Some(first)], Some(integrator)),
  );

  let read = listed(&fixture, id);
  let named: Vec<(usize, Option<ResourceId>)> = found(&read)
    .into_iter()
    .map(|problem| match problem {
      Problem::Step(step) => (step.number, step.blocked.names()),
      Problem::Member(member) => (0, Some(member.id)),
      other => panic!("steps and profiles only, got {other:?}"),
    })
    .collect();
  assert_eq!(
    named,
    [
      (1, Some(first)),
      (2, Some(second)),
      (3, Some(first)),
      (0, Some(integrator))
    ],
    "each step that runs a lost profile, then the integrator no step runs, as itself"
  );
}

#[test]
fn readiness_pipeline_ready_a_draft_step_problem_stands_beside_a_step_whose_profile_lost_its_connection() {
  let fixture = Fixture::local();
  let lost = lost_profile(&fixture, "lost");
  let id = fixture.own("stuck", &pipeline(&[Some(lost)], None));
  save_draft(&fixture, id, &pipeline(&[Some(lost), None], None));

  let read = listed(&fixture, id);
  let all = found(&read);
  assert!(
    matches!(
      all.as_slice(),
      [Problem::Step(gone), Problem::Step(empty)]
        if gone.number == 1 && gone.blocked.names() == Some(lost) && empty.blocked == Blocked::NoProfile
    ),
    "the draft's steps, each with what it lacks, and the profile not again: {all:?}"
  );
}

#[test]
fn readiness_pipeline_ready_a_profile_of_the_version_the_draft_no_longer_runs_is_left_out() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let lost = lost_profile(&fixture, "lost");
  let id = fixture.own("stuck", &pipeline(&[Some(builder)], Some(lost)));
  save_draft(&fixture, id, &pipeline(&[Some(builder)], None));

  let read = listed(&fixture, id);
  let basis = |runs: Option<&[ResourceId]>| {
    problems(Basis {
      flow: &read.flow,
      standing: read.standing.as_ref(),
      undecided: &read.undecided,
      published: true,
      runs,
    })
  };
  assert!(
    matches!(basis(None).as_slice(), [Problem::Member(member)] if member.id == lost),
    "the version's integrator cannot be told: {:?}",
    basis(None)
  );
  assert_eq!(
    basis(Some(&[builder])),
    [],
    "the draft no longer merges with it, so it is not the draft's problem"
  );
  assert!(
    matches!(basis(Some(&[builder, lost])).as_slice(), [Problem::Member(member)] if member.id == lost),
    "a draft that still runs it keeps it"
  );
}

/// Saves `body` as the draft of the published pipeline `id`.
fn save_draft(fixture: &Fixture, id: ResourceId, body: &Pipeline) {
  let version = fixture
    .db()
    .latest_version::<Pipeline>(id)
    .expect("read")
    .expect("published")
    .number();
  fixture
    .db()
    .save_draft(id, &StoredDraft::new(body.clone(), Some(version), at(30)))
    .expect("a draft");
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
    step_problems(&read.flow)[0].blocked,
    Blocked::Gone {
      id: unknown,
      tool: false
    },
    "a profile neither published nor drafted here is gone, not waiting to be published"
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
fn readiness_pipeline_ready_a_draft_a_line_cannot_draw_still_names_each_step_that_cannot_run() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let mut steps = pipeline(&[Some(builder), None, Some(builder)], None).into_steps();
  let first = steps[0].key.clone();
  steps[1].depends_on.insert(first);
  let id = draft(&fixture, "branching", &Pipeline::new(steps).expect("a valid pipeline"));

  let read = listed(&fixture, id);
  let all = found(&read);
  assert!(
    matches!(
      all.as_slice(),
      [Problem::Draft(Draft::Branches), Problem::Step(step)]
        if step.number == 2 && step.blocked == Blocked::NoProfile && &*step.name == "Step 1"
    ),
    "the layout, then the step the list line names, numbered in the order of the steps: {all:?}"
  );
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

#[test]
fn readiness_pipeline_ready_an_edit_the_page_holds_is_read_as_the_page_shows_it() {
  use orchester_core::resource::PipelineEdit;
  use orchester_workspace::pipelines::flow::FlowRead;

  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let id = fixture.own("delivery", &pipeline(&[Some(builder), Some(builder)], None));
  let hub = fixture
    .workspace
    .pipeline_hub(&fixture.root, &fixture.vault)
    .expect("the hub");
  let read = listed(&fixture, id);
  assert!(found(&read).is_empty(), "the stored version can run");
  let read_of = |edit: &PipelineEdit| FlowRead::of_edit(edit, &hub.assignables());
  let basis = |flow: &FlowRead| {
    problems(Basis {
      flow,
      standing: read.standing.as_ref(),
      undecided: &read.undecided,
      published: true,
      runs: None,
    })
  };

  // A step added to the edit that has nothing to run is listed before anything is saved.
  let mut edit = PipelineEdit::new(read.body.as_ref().clone());
  edit.add_step(name("Review"), None);
  assert!(
    matches!(basis(&read_of(&edit)).as_slice(), [Problem::Step(step)] if step.blocked.is_unassigned()),
    "{:?}",
    basis(&read_of(&edit))
  );
  assert!(found(&read).is_empty(), "the stored version is untouched");

  // An edit that changes nothing lists what the stored version does, and an edit that empties the pipeline says so.
  assert!(basis(&read_of(&PipelineEdit::new(read.body.as_ref().clone()))).is_empty());
  let mut emptied = PipelineEdit::new(read.body.as_ref().clone());
  for step in read.body.steps() {
    emptied.remove_step(&step.key);
  }
  assert_eq!(basis(&read_of(&emptied)), [Problem::Draft(Draft::NoSteps)]);
}

#[test]
fn readiness_pipeline_ready_a_step_that_names_an_archived_profile_is_listed_once() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let retired = fixture.own("retired", &profile_on(connection, &[]));
  let id = fixture.own("delivery", &pipeline(&[Some(builder), Some(retired)], None));
  fixture.db().archive_local_resource(retired, at(40)).expect("archived");

  let read = listed(&fixture, id);
  assert!(
    matches!(read.standing, Some(ReadinessStanding::Undecided(Undecided::RunsArchived(named, _))) if named == retired),
    "the readiness names the archived profile: {:?}",
    read.standing
  );
  assert_eq!(
    found(&read),
    [Problem::Step(step_problems(&read.flow).remove(0))],
    "the step that runs it says so; the readiness's own row is the same problem"
  );
  assert_eq!(
    step_problems(&read.flow)[0].blocked,
    Blocked::Gone {
      id: retired,
      tool: false
    }
  );
}

#[test]
fn readiness_pipeline_ready_unpublishable_says_what_publishing_refuses_before_the_press() {
  use orchester_core::resource::{PipelineEdit, RepositoryAccess, StepKind};
  use orchester_workspace::pipelines::{
    flow::FlowRead,
    local_edit::{Unpublishable, unpublishable},
  };

  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let reader = fixture.own("reader", &profile_on(connection, &[]));
  let id = fixture.own("delivery", &pipeline(&[Some(reader)], None));
  let hub = fixture
    .workspace
    .pipeline_hub(&fixture.root, &fixture.vault)
    .expect("the hub");
  let read = listed(&fixture, id);
  let mut edit = PipelineEdit::new(read.body.as_ref().clone());
  assert_eq!(
    unpublishable(&edit, &FlowRead::of_edit(&edit, &hub.assignables())),
    None
  );

  // The step asks to write with a profile that only reads: publishing refuses it, and the page knows before Save.
  let key = edit.steps()[0].key.clone();
  edit.update(&key, |step| {
    if let StepKind::Agent { repository, .. } = &mut step.kind {
      *repository = RepositoryAccess::Write;
    }
  });
  assert!(
    matches!(
      unpublishable(&edit, &FlowRead::of_edit(&edit, &hub.assignables())),
      Some(Unpublishable::Refused(why)) if why.key == "appMcp.errors.stepCannotWrite"
    ),
    "the refusal publishing gives"
  );
}
