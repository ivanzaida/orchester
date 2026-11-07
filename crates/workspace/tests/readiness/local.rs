//! A local project: the connection a profile names is its pick, and what the project owns counts as on.

use orchester_core::{
  connection::AgentProvider,
  id::ConnectionId,
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings},
};
use orchester_workspace::readiness::{Group, Standing, Undecided};

use crate::support::{Fixture, name, pipeline};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

fn profile_on(connection: ConnectionId) -> AgentProfile {
  AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    connection,
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
}

#[test]
fn readiness_a_local_profile_runs_on_its_own_connection_even_when_there_are_several_of_the_kind() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "work");
  fixture.connection(CLAUDE, "personal");
  let id = fixture.own("builder", &profile_on(own));
  let pipeline = fixture.own("delivery", &pipeline(&[Some(id)], Some(id)));

  let read = fixture.read();
  assert_eq!(read.resource(id).expect("listed").standing(), &Standing::Ready);
  assert_eq!(read.resource(id).expect("listed").group(), Group::Ready);
  assert_eq!(read.pipeline(pipeline).expect("listed").standing(), &Standing::Ready);
}

#[test]
fn readiness_a_local_profile_on_a_connection_the_project_does_not_have_is_undecided() {
  let fixture = Fixture::local();
  let gone = ConnectionId::generate();
  let id = fixture.own("builder", &profile_on(gone));

  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &Standing::Undecided(Undecided::ConnectionGone(gone))
  );
}
