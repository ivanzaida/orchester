//! A local project that archived a profile or an MCP server (ORC-842): the pipeline that names the profile stays and is
//! not ready with the archived profile named, a profile that chooses an archived server is not ready with the server
//! named, and restoring puts both back as they were.

use orchester_core::{connection::AgentProvider, resource::ResourceKind};
use orchester_workspace::readiness::{Group, ReadinessStanding, Undecided};

use crate::support::{Fixture, at, pipeline, profile_on};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

#[test]
fn readiness_local_archive_a_pipeline_keeps_naming_an_archived_profile_and_says_so() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "claude");
  let builder = fixture.own("builder", &profile_on(own, &[]));
  let delivery = fixture.own("delivery", &pipeline(&[Some(builder)], None));
  assert_eq!(
    fixture.read().pipeline(delivery).expect("listed").standing(),
    &ReadinessStanding::Ready
  );

  fixture.db().archive_local_resource(builder, at(20)).expect("archived");
  let read = fixture.read();
  assert!(read.resource(builder).is_none(), "an archived profile leaves the list");
  assert_eq!(
    read.pipeline(delivery).expect("still listed").standing(),
    &ReadinessStanding::Undecided(Undecided::RunsArchived(builder, "builder".to_owned())),
    "the pipeline is not ready, and names what is archived"
  );

  fixture.db().restore_local_resource(builder).expect("restored");
  let read = fixture.read();
  assert_eq!(read.resource(builder).expect("back").group(), Group::Ready);
  assert_eq!(
    read.pipeline(delivery).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
  assert!(
    fixture
      .db()
      .archived_local_resources(ResourceKind::AgentProfile)
      .expect("archived")
      .is_empty()
  );
}

#[test]
fn readiness_local_archive_a_profile_choosing_an_archived_server_is_not_ready_and_names_it() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "claude");
  let file = fixture.program_outside("knowledge.sh");
  let server = fixture.local_server("knowledge", file.to_str().expect("utf-8"));
  let builder = fixture.own("builder", &profile_on(own, &[server]));
  let delivery = fixture.own("delivery", &pipeline(&[Some(builder)], None));
  assert_eq!(
    fixture.read().resource(builder).expect("listed").standing(),
    &ReadinessStanding::Ready
  );

  fixture.db().archive_mcp_server(server, at(20)).expect("archived");
  let why = Undecided::ChoosesArchivedLocalMcpServer(server, "knowledge".to_owned());
  let read = fixture.read();
  assert_eq!(
    read.resource(builder).expect("listed").standing(),
    &ReadinessStanding::Undecided(why.clone())
  );
  assert_eq!(read.resource(builder).expect("listed").group(), Group::NeedsYouHere);
  assert_eq!(
    read.pipeline(delivery).expect("listed").standing(),
    &ReadinessStanding::Undecided(Undecided::Member {
      id: builder,
      why: Box::new(why)
    })
  );

  fixture.db().restore_mcp_server(server).expect("restored");
  assert_eq!(
    fixture.read().resource(builder).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
}
