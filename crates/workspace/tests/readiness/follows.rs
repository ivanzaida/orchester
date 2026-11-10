//! The read keeps nothing, so it follows the fill-ins and the latest versions the team server was read with.

use orchester_core::{
  connection::AgentProvider,
  resource::VersionNumber,
  sharing::{ConnectionProblem, Missing},
};
use orchester_workspace::readiness::ReadinessStanding;

use crate::support::{Fixture, profile};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;
const CODEX: AgentProvider = AgentProvider::CodexCli;

#[test]
fn readiness_follows_a_new_connection_and_a_new_version() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let id = fixture.team_profile("builder", &profile(CLAUDE, &[], &[]));
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Ready
  );

  let second = VersionNumber::FIRST.next().expect("v2");
  fixture.next_profile_version(id, second, &profile(CODEX, &[], &[]));
  let read = fixture.read();
  let found = read.resource(id).expect("listed");
  assert_eq!(
    found.number(),
    second,
    "the newest version a person did not have to confirm"
  );
  assert_eq!(
    found.standing().first(),
    Some(&Missing::Connection {
      profile: id,
      cli: CODEX,
      problem: ConnectionProblem::NoneOfKind
    })
  );

  fixture.connection(CODEX, "codex");
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
}
