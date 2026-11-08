//! A local project: the connection a profile names is its pick, and what the project owns counts as on.

use orchester_core::{
  connection::{AgentProvider, mcp::McpServerName},
  id::ConnectionId,
  sharing::{LocalProgramProblem, Missing},
};
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
  let file = fixture.program_outside("knowledge.sh");
  let found = fixture.local_server("found", file.to_str().expect("utf-8"));
  let missing = fixture.dir_path().join("missing.sh");
  let gone = fixture.local_server("gone", missing.to_str().expect("utf-8"));
  let id = fixture.own("builder", &profile_on(own, &[found, gone]));

  let read = fixture.read();
  let listed = read.resource(id).expect("listed");
  assert_eq!(
    listed.standing().missing(),
    [Missing::LocalMcpProgram {
      server: gone,
      name: McpServerName::new("gone").expect("name"),
      problem: LocalProgramProblem::NotFound,
    }]
  );
  assert_eq!(listed.group(), Group::NeedsYouHere);
  assert_eq!(read.resources().len(), 1, "a local server is not a resource of its own");

  std::fs::write(&missing, "#!/bin/sh\n").expect("the program appears");
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Ready,
    "read again, the file that appeared is found"
  );
}

#[test]
fn readiness_a_fixed_program_inside_the_repository_is_moved_as_a_run_refuses_it() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "claude");
  let inside = fixture.program_file("planted.sh");
  let planted = fixture.local_server("planted", inside.to_str().expect("utf-8"));
  let id = fixture.own("builder", &profile_on(own, &[planted]));

  assert_eq!(
    fixture.read().resource(id).expect("listed").standing().missing(),
    [Missing::LocalMcpProgram {
      server: planted,
      name: McpServerName::new("planted").expect("name"),
      problem: LocalProgramProblem::Moved,
    }]
  );
}

#[test]
fn readiness_a_server_turned_off_is_left_out_as_a_run_leaves_it_out() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "claude");
  let missing = fixture.dir_path().join("missing.sh");
  let off = fixture.local_server("off", missing.to_str().expect("utf-8"));
  let id = fixture.own("builder", &profile_on(own, &[off]));
  assert!(
    !fixture
      .read()
      .resource(id)
      .expect("listed")
      .standing()
      .missing()
      .is_empty()
  );

  fixture.disable_server(off);
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
}

#[test]
fn readiness_a_program_found_only_on_the_projects_path_is_ready() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "claude");
  let folder = fixture.dir_path().join("tools");
  std::fs::create_dir_all(&folder).expect("folder");
  let name = "orchester-readiness-only-here";
  let server = fixture.local_server("named", name);
  let id = fixture.own("builder", &profile_on(own, &[server]));
  assert!(
    !fixture
      .read()
      .resource(id)
      .expect("listed")
      .standing()
      .missing()
      .is_empty()
  );

  std::fs::write(folder.join(name), "#!/bin/sh\n").expect("the program");
  fixture.project_path(&folder);
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Ready
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
