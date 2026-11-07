//! A version of the Admin's own that waits for the server is read in the team's form, as publishing sends it.

use orchester_core::{
  connection::AgentProvider,
  resource::{TeamProfileError, VersionNumber},
  sharing::{ConnectionProblem, Missing},
};
use orchester_workspace::readiness::{Group, ReadinessStanding, Undecided};

use crate::support::{Fixture, pipeline, profile, profile_on};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

#[test]
fn readiness_an_offline_published_version_of_a_ready_profile_stays_ready() {
  let fixture = Fixture::team();
  let claude = fixture.connection(CLAUDE, "claude");
  let id = fixture.team_profile("builder", &profile(CLAUDE, &[], &[]));
  let second = fixture.publish_offline(id, profile_on(claude, &[]));

  let read = fixture.read();
  let found = read.resource(id).expect("listed");
  assert_eq!(second, VersionNumber::FIRST.next().expect("v2"));
  assert_eq!(
    found.number(),
    second,
    "the version this computer uses is the pending one"
  );
  assert_eq!(found.standing(), &ReadinessStanding::Ready);
  assert_eq!(found.group(), Group::Ready);
}

#[test]
fn readiness_a_new_pending_profile_and_the_pipeline_running_it_are_ready_when_the_facts_are() {
  let fixture = Fixture::team();
  let claude = fixture.connection(CLAUDE, "claude");
  let id = fixture.pending_profile("builder", &profile_on(claude, &[]));
  let flow = fixture.team_resource("flow", pipeline(&[Some(id)], Some(id)));

  let read = fixture.read();
  assert_eq!(read.resource(id).expect("listed").standing(), &ReadinessStanding::Ready);
  assert_eq!(
    read.pipeline(flow).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
}

#[test]
fn readiness_a_pending_profile_needs_what_its_cli_kind_needs_here() {
  let fixture = Fixture::team();
  let work = fixture.connection(CLAUDE, "work");
  fixture.connection(CLAUDE, "personal");
  let id = fixture.pending_profile("builder", &profile_on(work, &[]));
  let flow = fixture.team_resource("flow", pipeline(&[Some(id)], None));

  let pick_needed = [Missing::Connection {
    profile: id,
    cli: CLAUDE,
    problem: ConnectionProblem::PickNeeded,
  }];
  let read = fixture.read();
  assert_eq!(read.resource(id).expect("listed").standing().missing(), pick_needed);
  assert_eq!(read.pipeline(flow).expect("listed").standing().missing(), pick_needed);
}

#[test]
fn readiness_a_pending_profile_that_cannot_be_published_is_undecided_with_the_reason() {
  let fixture = Fixture::team();
  let claude = fixture.connection(CLAUDE, "claude");
  let local = fixture.local_server("knowledge", "npx");
  let id = fixture.pending_profile("builder", &profile_on(claude, &[local]));
  let flow = fixture.team_resource("flow", pipeline(&[Some(id)], None));

  let why = Undecided::CannotBePublished(TeamProfileError::LocalMcpServer("knowledge".to_owned()));
  let read = fixture.read();
  assert_eq!(
    read.resource(id).expect("listed").standing(),
    &ReadinessStanding::Undecided(why.clone())
  );
  assert_eq!(
    read.pipeline(flow).expect("listed").standing(),
    &ReadinessStanding::Undecided(Undecided::Member { id, why: Box::new(why) }),
    "the pipeline names the member that cannot be decided"
  );
}
