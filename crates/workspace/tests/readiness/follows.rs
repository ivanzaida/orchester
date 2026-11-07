//! The read keeps nothing, so it follows the fill-ins and the versions the stream caches.

use orchester_core::{
  connection::AgentProvider,
  resource::VersionNumber,
  sharing::{ConnectionProblem, Missing},
};
use orchester_workspace::readiness::Standing;

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
    &Standing::Ready
  );

  let second = VersionNumber::FIRST.next().expect("v2");
  fixture.next_profile_version(id, second, &profile(CODEX, &[], &[]), false);
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
    &Standing::Ready
  );
}

#[test]
fn readiness_is_about_the_version_this_computer_uses_not_one_that_waits_for_the_person() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let id = fixture.team_profile("builder", &profile(CLAUDE, &[], &[]));

  let second = VersionNumber::FIRST.next().expect("v2");
  fixture.next_profile_version(id, second, &profile(CODEX, &[], &[]), true);
  let read = fixture.read();
  let found = read.resource(id).expect("listed");
  assert_eq!(found.number(), VersionNumber::FIRST);
  assert_eq!(
    found.standing(),
    &Standing::Ready,
    "v2 would need a Codex connection, but v1 runs"
  );
}
