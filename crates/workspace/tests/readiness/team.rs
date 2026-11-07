//! A team profile's connection: by CLI kind, and by the person's pick when there are several.

use orchester_core::{
  connection::AgentProvider,
  sharing::{ConnectionProblem, Missing},
};
use orchester_store::ConnectionPick;
use orchester_workspace::readiness::{Group, ReadinessStanding};

use crate::support::{Fixture, profile};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

#[test]
fn readiness_a_team_profile_with_no_connection_of_its_kind_says_to_add_one() {
  let fixture = Fixture::team();
  let id = fixture.team_profile("builder", &profile(CLAUDE, &[], &[]));
  fixture.connection(AgentProvider::CodexCli, "codex");

  let read = fixture.read();
  let found = read.resource(id).expect("listed");
  assert_eq!(found.group(), Group::NeedsYouHere);
  assert_eq!(
    found.standing().first(),
    Some(&Missing::Connection {
      profile: id,
      cli: CLAUDE,
      problem: ConnectionProblem::NoneOfKind
    }),
    "a connection of another kind does not count"
  );
  assert_eq!(found.number().get(), 1);
  assert_eq!(found.name(), "builder");
}

#[test]
fn readiness_the_only_connection_of_the_kind_is_used_without_asking() {
  let fixture = Fixture::team();
  let id = fixture.team_profile("builder", &profile(CLAUDE, &[], &[]));
  fixture.connection(CLAUDE, "claude");

  let read = fixture.read();
  assert_eq!(read.resource(id).expect("listed").standing(), &ReadinessStanding::Ready);
  assert_eq!(read.resource(id).expect("listed").group(), Group::Ready);
}

#[test]
fn readiness_several_connections_of_the_kind_need_a_pick_and_the_pick_makes_it_ready() {
  let fixture = Fixture::team();
  let id = fixture.team_profile("builder", &profile(CLAUDE, &[], &[]));
  let work = fixture.connection(CLAUDE, "work");
  fixture.connection(CLAUDE, "personal");

  let read = fixture.read();
  assert_eq!(
    read.resource(id).expect("listed").standing().first(),
    Some(&Missing::Connection {
      profile: id,
      cli: CLAUDE,
      problem: ConnectionProblem::PickNeeded
    })
  );

  fixture
    .workspace
    .pick_team_connection(
      &fixture.root,
      id,
      ConnectionPick {
        connection: work,
        account: None,
      },
    )
    .expect("pick");
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
}
