//! What keeps a pipeline of the hub from running as it is, as the page's *Ready?* lists it (`Pipelines · v2` P1, P8):
//! what is wrong with the steps as a whole, the steps that cannot run, then what the readiness read says, then why it
//! cannot be told. A pipeline is stored only as published versions (D-032): the page reads the latest version, or the
//! edit it holds over it.

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId},
  resource::{FlowError, Pipeline, PipelineEdit, PipelineError, StepKey},
  sharing::Missing,
};
use orchester_workspace::{
  pipelines::{
    flow::FlowRead,
    hub::HubPipeline,
    ready::{Basis, Blocked, Problem, Whole, problems, step_problems},
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

/// The flow of an edit of the pipeline that holds `body`, as the page reads it while it holds the edit.
fn edited(fixture: &Fixture, body: &Pipeline) -> FlowRead {
  let hub = fixture
    .workspace
    .pipeline_hub(&fixture.root, &fixture.vault)
    .expect("the hub");
  FlowRead::of_edit(&PipelineEdit::new(body.clone()), &hub.assignables())
}

fn found(pipeline: &HubPipeline) -> Vec<Problem> {
  found_in(pipeline, &pipeline.flow, None)
}

/// What *Ready?* lists of `flow` for `pipeline`, leaving out what `runs` does not run when it is given.
fn found_in(pipeline: &HubPipeline, flow: &FlowRead, runs: Option<&[ResourceId]>) -> Vec<Problem> {
  problems(Basis {
    flow,
    standing: pipeline.standing.as_ref(),
    undecided: &pipeline.undecided,
    runs,
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
fn readiness_pipeline_ready_an_edit_names_each_step_that_has_nothing_to_run() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let id = fixture.own("delivery", &pipeline(&[Some(builder)], None));
  let read = listed(&fixture, id);
  let flow = edited(&fixture, &pipeline(&[Some(builder), None, None], None));

  let steps = step_problems(&flow);
  assert_eq!(steps.len(), 2);
  assert_eq!(
    steps.iter().map(|step| step.number).collect::<Vec<_>>(),
    [2, 3],
    "in the order of the flow, by the number the card carries"
  );
  assert_eq!(&*steps[0].name, "Step 1");
  assert!(steps.iter().all(|step| step.blocked == Blocked::NoProfile));
  assert_eq!(
    found_in(&read, &flow, None),
    steps.into_iter().map(Problem::Step).collect::<Vec<_>>(),
    "the edit's problems are its steps"
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
fn readiness_pipeline_ready_an_edit_step_problem_stands_beside_a_step_whose_profile_lost_its_connection() {
  let fixture = Fixture::local();
  let lost = lost_profile(&fixture, "lost");
  let id = fixture.own("stuck", &pipeline(&[Some(lost)], None));
  let read = listed(&fixture, id);
  let flow = edited(&fixture, &pipeline(&[Some(lost), None], None));

  let all = found_in(&read, &flow, None);
  assert!(
    matches!(
      all.as_slice(),
      [Problem::Step(gone), Problem::Step(empty)]
        if gone.number == 1 && gone.blocked.names() == Some(lost) && empty.blocked == Blocked::NoProfile
    ),
    "the edit's steps, each with what it lacks, and the profile not again: {all:?}"
  );
}

#[test]
fn readiness_pipeline_ready_a_profile_of_the_version_the_edit_no_longer_runs_is_left_out() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let lost = lost_profile(&fixture, "lost");
  let id = fixture.own("stuck", &pipeline(&[Some(builder)], Some(lost)));
  let read = listed(&fixture, id);
  let flow = edited(&fixture, &pipeline(&[Some(builder)], None));

  let basis = |runs: Option<&[ResourceId]>| found_in(&read, &flow, runs);
  assert!(
    matches!(basis(None).as_slice(), [Problem::Member(member)] if member.id == lost),
    "the version's integrator cannot be told: {:?}",
    basis(None)
  );
  assert_eq!(
    basis(Some(&[builder])),
    [],
    "the edit no longer merges with it, so it is not the edit's problem"
  );
  assert!(
    matches!(basis(Some(&[builder, lost])).as_slice(), [Problem::Member(member)] if member.id == lost),
    "an edit that still runs it keeps it"
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
    step_problems(&read.flow)[0].blocked,
    Blocked::Gone {
      id: unknown,
      tool: false
    },
    "a profile neither published nor drafted here is gone, not waiting to be published"
  );
}

#[test]
fn readiness_pipeline_ready_an_edit_with_no_steps_says_so() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let id = fixture.own("delivery", &pipeline(&[Some(builder)], None));
  let read = listed(&fixture, id);

  let flow = edited(&fixture, &pipeline(&[], None));
  assert_eq!(found_in(&read, &flow, None), [Problem::Whole(Whole::NoSteps)]);
}

/// Three steps of `builder` (or none where `None`) whose second waits for only the first, though the third also
/// starts the run: a branch one line cannot draw.
fn branching(builder: [Option<ResourceId>; 3]) -> Pipeline {
  let mut steps = pipeline(&builder, None).into_steps();
  let first = steps[0].key.clone();
  steps[1].depends_on.insert(first);
  Pipeline::new(steps).expect("a valid pipeline")
}

#[test]
fn readiness_pipeline_ready_a_pipeline_a_line_cannot_draw_says_so() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  // A version an older build published; a save refuses such steps now (`pipelines.problems.branches`).
  let id = fixture.own("branching", &branching([Some(builder); 3]));

  let read = listed(&fixture, id);
  assert_eq!(found(&read), [Problem::Whole(Whole::Branches)]);
}

#[test]
fn readiness_pipeline_ready_an_edit_a_line_cannot_draw_still_names_each_step_that_cannot_run() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let id = fixture.own("delivery", &pipeline(&[Some(builder)], None));
  let read = listed(&fixture, id);
  let flow = edited(&fixture, &branching([Some(builder), None, Some(builder)]));

  let all = found_in(&read, &flow, None);
  assert!(
    matches!(
      all.as_slice(),
      [Problem::Whole(Whole::Branches), Problem::Step(step)]
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
  let id = fixture.own("cycle", &pipeline(&[Some(builder)], None));
  let mut read = listed(&fixture, id);
  // A stored pipeline cannot hold a cycle (`Pipeline::new` refuses it), so the flow's failure is put in by hand.
  let key = StepKey::new("step-0").expect("key");
  let flow = std::sync::Arc::make_mut(&mut read.flow);
  flow.flow = Err(FlowError::Broken(PipelineError::Cycle(key)));

  assert_eq!(found(&read), [Problem::Whole(Whole::Broken)]);
}

#[test]
fn readiness_pipeline_ready_an_edit_lists_its_own_steps_while_the_list_speaks_of_the_version() {
  let fixture = Fixture::local();
  let connection = fixture.connection(CLAUDE, "work");
  let builder = fixture.own("builder", &profile_on(connection, &[]));
  let id = fixture.own("delivery", &pipeline(&[Some(builder), Some(builder)], None));
  let read = listed(&fixture, id);
  assert!(found(&read).is_empty(), "the version tasks use can run");

  let flow = edited(&fixture, &pipeline(&[Some(builder), None], None));
  assert_eq!(read.group(), Group::Ready, "the list speaks of the version tasks use");
  assert!(
    matches!(found_in(&read, &flow, None).as_slice(), [Problem::Step(step)] if step.number == 2),
    "the page describes the edit: {:?}",
    found_in(&read, &flow, None)
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
  assert_eq!(basis(&read_of(&emptied)), [Problem::Whole(Whole::NoSteps)]);
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
