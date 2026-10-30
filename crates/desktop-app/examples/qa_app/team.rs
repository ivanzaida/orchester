//! The stand-in team server of the `ownership-signed-in` scenario: joined with a code while signed in, so it is in the
//! account's list, and it lists one project, whose repository is Ledger's. What a local project's Share page says it
//! moves needs a server of the account's that answers. Fixture data for QA, deterministic and never shipped.

use std::error::Error;

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
  test_support::team_server::{CODE, TeamServer, USERNAME},
};
use orchester_app::services::Services;
use orchester_workspace::sharing::{CodeRequest, Trust, team_join::JoinOutcome};
use tokio::runtime::Runtime;

/// Starts the server and joins it from the session of `services`; it answers while the returned value is kept.
pub fn join(services: &Services, runtime: &Runtime) -> Result<TeamServer, Box<dyn Error>> {
  let team = TeamServer::start();
  team.share(&[("Ledger", "ledger", Some("github.com/acme/ledger"))]);
  let address = ServerAddress::parse(&team.url())?;
  let request = CodeRequest {
    code: Credential::new(CODE.to_owned()),
    username: Some(USERNAME.to_owned()),
    display_name: Some("Bohdan Koval".to_owned()),
    proof: None,
  };
  // Plain HTTP to this computer needs no opt-in.
  let trust = Trust::System(PlainHttp::NotAllowed);
  match runtime.block_on(services.team_join.join_by_code(&address, trust, request))? {
    JoinOutcome::Joined(_) => Ok(team),
    JoinOutcome::Waiting(_) | JoinOutcome::NotInvited => Err("the stand-in team server took no join".into()),
  }
}
