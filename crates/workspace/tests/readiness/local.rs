//! A local project: the connection a profile names is its pick, and what the project owns counts as on.

use orchester_core::{connection::AgentProvider, id::ConnectionId, sharing::Missing};
use orchester_workspace::readiness::{Group, ReadinessStanding, Undecided};

use crate::support::{Fixture, pipeline, profile_on};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

#[test]
fn readiness_a_local_profile_runs_on_its_own_connection_even_when_there_are_several_of_the_kind() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "work");
  fixture.connection(CLAUDE, "personal");
  let id = fixture.own("builder", &profile_on(own, &[]));
  let pipeline = fixture.own("delivery", &pipeline(&[Some(id)], Some(id)));

  let read = fixture.read();
  assert_eq!(read.resource(id).expect("listed").standing(), &ReadinessStanding::Ready);
  assert_eq!(read.resource(id).expect("listed").group(), Group::Ready);
  assert_eq!(
    read.pipeline(pipeline).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
}

#[test]
fn readiness_a_local_profile_on_a_connection_the_project_does_not_have_is_undecided() {
  let fixture = Fixture::local();
  let gone = ConnectionId::generate();
  let id = fixture.own("builder", &profile_on(gone, &[]));

  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Undecided(Undecided::ConnectionGone(gone))
  );
}

#[test]
fn readiness_a_local_profile_needs_the_program_of_each_server_it_chooses_to_be_a_file() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "claude");
  let file = fixture.program_file("knowledge.sh");
  let found = fixture.local_server("found", file.to_str().expect("utf-8"));
  let gone = fixture.local_server("gone", fixture.root.join("missing.sh").to_str().expect("utf-8"));
  let id = fixture.own("builder", &profile_on(own, &[found, gone]));

  let read = fixture.read();
  let listed = read.resource(id).expect("listed");
  assert_eq!(listed.standing().missing(), [Missing::LocalMcpProgram { server: gone }]);
  assert_eq!(listed.group(), Group::NeedsYouHere);
  assert_eq!(read.resources().len(), 1, "a local server is not a resource of its own");

  std::fs::write(fixture.root.join("missing.sh"), "#!/bin/sh\n").expect("the program appears");
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Ready,
    "read again, the file that appeared is found"
  );
}

#[test]
fn readiness_a_server_the_profile_chooses_that_the_project_no_longer_has_is_left_to_the_run() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "claude");
  let id = fixture.own("builder", &profile_on(own, &[ConnectionId::generate()]));

  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
}
