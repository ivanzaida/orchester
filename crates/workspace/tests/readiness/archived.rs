//! What an archive of a team resource leaves on this computer (`docs/design/team-configuration.md`, edge case 8, and
//! A22 `pUu78`): a profile that still chooses the archived MCP server cannot run, so Run again is refused as not ready
//! here until an admin restores the server or the profile stops choosing it.

use orchester_core::{connection::AgentProvider, run::Answerer, time::Timestamp};
use orchester_workspace::readiness::StartReadiness;

use crate::support::{Fixture, pipeline, profile, program};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

#[test]
fn readiness_archived_run_again_is_refused_while_a_profile_still_chooses_the_archived_server() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let docs = fixture.team_server("knowledge", program(&[]));
  let builder = fixture.team_profile("builder", &profile(CLAUDE, &[docs], &[]));
  let delivery = fixture.team_resource("delivery", pipeline(&[Some(builder)], None));
  let work = fixture.workspace.clone();
  work
    .switch_team_mcp_on_here(&fixture.root, docs, true, Answerer::Person)
    .expect("on");
  let path = fixture.program_file("knowledge-server");
  work
    .set_team_mcp_program(&fixture.root, docs, &path, Answerer::Person)
    .expect("program");
  let ready = work
    .team_start_readiness(&fixture.root, &fixture.vault, delivery)
    .expect("read");
  assert!(
    matches!(ready, StartReadiness::Ready(_)),
    "ready before the archive: {ready:?}"
  );

  // The server's archive reaches this computer through its event stream, which removes the resource here.
  fixture
    .db()
    .mark_shared_archived(docs, true, Timestamp::now())
    .expect("archived here");
  let read = work
    .team_start_readiness(&fixture.root, &fixture.vault, delivery)
    .expect("read");
  let StartReadiness::NotReady(refusal) = read else {
    panic!("Run again went on with Builder choosing an archived server: {read:?}");
  };
  let keys: Vec<&str> = refusal.messages().iter().map(|message| message.key).collect();
  assert!(
    keys.contains(&"notReadyHere.undecidedChoosesMcpServer"),
    "the refusal says Builder chooses a server the team does not offer: {keys:?}"
  );

  // An admin restores it: the server's `resource.archived` with `archived` false brings it back.
  fixture
    .db()
    .mark_shared_archived(docs, false, Timestamp::now())
    .expect("restored here");
  let again = work
    .team_start_readiness(&fixture.root, &fixture.vault, delivery)
    .expect("read");
  assert!(
    matches!(again, StartReadiness::Ready(_)),
    "ready once restored: {again:?}"
  );
}
