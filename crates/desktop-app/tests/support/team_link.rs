//! Shares a project with the stand-in team server on loopback, its owner being the person here, signed in to it: what
//! goes to the team server at once is taken by it ([`share_on_stand_in`]).

use std::{collections::HashMap, path::Path};

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
  test_support::team_server::{CODE, Standing, TeamServer, USERNAME},
};
use orchester_core::{sharing::MemberRole, time::Timestamp};
use orchester_store::{ProjectDb, SharingLink};
use orchester_workspace::sharing::{CodeRequest, Trust, team_join::JoinOutcome};

use super::{Window, team_cache::keep_cached_team};

/// Shares the project at `repo`, open in `window`'s session, with the stand-in team server, its owner being the person
/// here, signed in to it by the server's code: what the team's pipelines save goes to that server (D-043). The server,
/// to hold while the window is open.
pub fn share_on_stand_in(window: &Window, repo: &Path) -> TeamServer {
  let server = TeamServer::named("Team");
  // The server takes an admin's writes only.
  server.stand_as(Standing::Admin);
  server.share(&[("Kontur", STAND_IN_SLUG, None)]);
  let services = window.launch.services().expect("the session");
  let address = ServerAddress::parse(&server.url()).expect("the stand-in's address");
  let request = CodeRequest {
    code: Credential::new(CODE.to_owned()),
    username: Some(USERNAME.to_owned()),
    display_name: None,
    proof: None,
  };
  let joined = window
    .data
    .runtime
    .block_on(
      services
        .team_join
        .join_by_code(&address, Trust::System(PlainHttp::NotAllowed), request),
    )
    .expect("the stand-in takes its code");
  let JoinOutcome::Joined(joined) = joined else {
    panic!("not joined: {joined:?}");
  };
  let link = SharingLink {
    server: joined.id,
    project: server.project(STAND_IN_SLUG).expect("the team shares Kontur"),
    key: ProjectDb::open(repo).expect("database").project().key().clone(),
    role: Some(MemberRole::Owner),
    joined_at: Timestamp::now(),
    cursor: 0,
    last_synced_at: Timestamp::now(),
    removed_at: None,
    address: None,
  };
  ProjectDb::open(repo)
    .expect("database")
    .start_sharing(&link, &[], (&[], &HashMap::new()))
    .expect("linked");
  keep_cached_team(window.launch.workspace().expect("the session"), repo);
  server
}

/// The slug of the stand-in team's Kontur.
pub const STAND_IN_SLUG: &str = "kontur";
