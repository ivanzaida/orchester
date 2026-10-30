//! Joining a team with no project open (`orchester_workspace::sharing::team_join`), as the start page and Settings ›
//! Orchester do: the Cloud account's team servers and invites, a join that gives this computer its own device sign-in,
//! a join by address and code with and without Cloud, the team's projects after joining, and the refusals in the
//! workspace's words; the returning start's All projects ([`returning`]); and a linked checkout's first download from
//! the stand-in ([`first_download`]). Run with `cargo test -p orchester-workspace --test startup_join startup_join`.

mod first_download;
mod returning;
mod support;

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
  test_support::{
    account_cloud::AccountCloud,
    team_server::{CODE, LATER_CODE, NAME, TeamServer, USERNAME},
  },
};
use orchester_workspace::{
  i18n::{FALLBACK_LOCALE, LOCALES},
  sharing::{
    CodeRequest, Trust,
    cloud::CloudError,
    team_join::{JoinError, JoinOutcome, JoinRefusal, JoinedServer},
  },
};
use serde_json::Value;
use support::{Desk, closed_address};

/// The team server's id at the stand-in cloud.
const AUDIENCE: &str = "0191c7c0-0000-7000-8000-0000000005e1";
/// Alice's invite to Billing, as the cloud and the team server name it.
const INVITE: &str = "0191c7c0-0000-7000-8000-00000000171e";

/// The stand-in cloud, naming the started team server as the account's.
fn cloud_with(server: &TeamServer) -> AccountCloud {
  let cloud = AccountCloud::start();
  cloud.name_server(AUDIENCE, &server.url(), None);
  cloud
}

fn address(server: &TeamServer) -> ServerAddress {
  ServerAddress::parse(&server.url()).expect("the team server's address")
}

/// Plain HTTP to this computer, which needs no opt-in.
fn trust() -> Trust {
  Trust::System(PlainHttp::NotAllowed)
}

fn code(text: &str) -> CodeRequest {
  CodeRequest {
    code: Credential::new(text.to_owned()),
    username: Some(USERNAME.to_owned()),
    display_name: Some("Bohdan Koval".to_owned()),
    proof: None,
  }
}

fn joined(outcome: Result<JoinOutcome, JoinError>) -> JoinedServer {
  match outcome {
    Ok(JoinOutcome::Joined(joined)) => joined,
    other => panic!("not joined: {other:?}"),
  }
}

fn refused<T>(result: Result<T, JoinError>) -> JoinRefusal {
  match result {
    Err(JoinError::Refused(refusal)) => refusal,
    Err(error) => panic!("not a refusal: {error:?}"),
    Ok(_) => panic!("it went through"),
  }
}

/// Joins `server` with its good code from `desk`.
fn join_by_code(desk: &Desk, server: &TeamServer) -> JoinedServer {
  joined(desk.run(desk.join.join_by_code(&address(server), trust(), code(CODE))))
}

#[test]
fn startup_join_lists_cloud_teams_and_invites() {
  let server = TeamServer::start();
  let cloud = cloud_with(&server);
  cloud.invite(INVITE, Some(("Billing", &format!("{}/billing", server.url()))));
  let desk = Desk::signed_in(&cloud);
  desk.assert_no_project();

  let teams = desk.run(desk.join.teams()).expect("the account's teams");

  assert_eq!(teams.servers.len(), 1);
  assert_eq!(teams.servers[0].server.id.to_string(), AUDIENCE);
  assert_eq!(teams.servers[0].server.name, NAME);
  assert_eq!(teams.servers[0].server.address, server.url());
  assert_eq!(teams.servers[0].joined, None, "this computer has joined nothing yet");
  let invites: Vec<_> = teams.invites.iter().map(|invite| invite.id.to_string()).collect();
  assert_eq!(invites, [INVITE]);
  assert_eq!(
    teams.invites[0].project.as_ref().map(|project| project.name.as_str()),
    Some("Billing")
  );

  let signed_out = Desk::new();
  let listed = signed_out.run(signed_out.join.teams());
  assert!(
    matches!(listed, Err(JoinError::Cloud(CloudError::SignedOut))),
    "{listed:?}"
  );
}

#[test]
fn startup_join_joins_a_listed_team_with_its_own_device_sign_in() {
  let server = TeamServer::start();
  let cloud = cloud_with(&server);
  let desk = Desk::signed_in(&cloud);
  let teams = desk.run(desk.join.teams()).expect("the account's teams");

  let team = joined(desk.run(desk.join.join_team(&teams.servers[0].server)));

  assert_eq!((team.name.as_str(), team.username.as_str()), (NAME, USERNAME));
  assert_eq!(server.cloud_joins(), 1);
  let token = desk.device_token(team.id).expect("the device token is in the keychain");
  assert!(token.starts_with("orc_"));
  let teams = desk.run(desk.join.teams()).expect("the account's teams");
  assert_eq!(teams.servers[0].joined, Some(team.id));
  desk.assert_no_project();
}

#[test]
fn startup_join_says_when_the_team_has_no_place_for_the_account() {
  let server = TeamServer::start();
  server.not_invited();
  let cloud = cloud_with(&server);
  let desk = Desk::signed_in(&cloud);
  let teams = desk.run(desk.join.teams()).expect("the account's teams");

  let outcome = desk.run(desk.join.join_team(&teams.servers[0].server));

  assert!(matches!(outcome, Ok(JoinOutcome::NotInvited)), "{outcome:?}");
  assert!(desk.entries().is_empty());
}

#[test]
fn startup_join_accepts_an_invite() {
  let server = TeamServer::start();
  let cloud = cloud_with(&server);
  cloud.invite(INVITE, Some(("Billing", &format!("{}/billing", server.url()))));
  let desk = Desk::signed_in(&cloud);
  let teams = desk.run(desk.join.teams()).expect("the account's teams");

  let team = joined(desk.run(desk.join.join_invite(&teams.invites[0])));

  assert_eq!(team.name, NAME);
  assert_eq!(cloud.tokens_for(INVITE), 1);
  assert!(desk.device_token(team.id).is_some());
  let teams = desk.run(desk.join.teams()).expect("the account's teams");
  assert!(teams.invites.is_empty(), "an accepted invite is listed no more");
  assert_eq!(teams.servers[0].joined, Some(team.id));
}

#[test]
fn startup_join_by_code_without_cloud() {
  let server = TeamServer::start();
  let desk = Desk::new();
  desk.assert_no_project();

  let reached = desk
    .run(desk.join.reach(&address(&server), PlainHttp::NotAllowed))
    .expect("the server answers");
  let team = join_by_code(&desk, &server);

  assert_eq!(reached.info.name, NAME);
  assert_eq!((team.name.as_str(), team.username.as_str()), (NAME, USERNAME));
  assert_eq!(server.code_joins(), [false], "without Cloud the code goes alone");
  assert!(desk.device_token(team.id).is_some());
  let entries = desk.entries();
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].stored.id, team.id);
  desk.assert_no_project();
}

#[test]
fn startup_join_by_code_signed_in_to_cloud() {
  let server = TeamServer::start();
  let cloud = cloud_with(&server);
  let desk = Desk::signed_in(&cloud);

  let team = join_by_code(&desk, &server);

  assert_eq!(team.username, USERNAME);
  assert!(desk.device_token(team.id).is_some());
  // The server is not linked to Orchester Cloud, so it is sent no cloud identity.
  assert_eq!(server.code_joins(), [false]);
  assert_eq!(desk.entries()[0].stored.id, team.id);
  desk.run(desk.cloud.sign_out()).expect("sign out");
  assert!(desk.entries().is_empty(), "the server went into the account's list");
}

#[test]
fn startup_join_refuses_a_wrong_code() {
  let server = TeamServer::start();
  let cloud = cloud_with(&server);
  for desk in [Desk::new(), Desk::signed_in(&cloud)] {
    let refusal = refused(desk.run(desk.join.join_by_code(&address(&server), trust(), code("WRONG123"))));

    assert_eq!(refusal, JoinRefusal::WrongCode);
    assert_eq!(refusal.message().key, "teamJoin.refused.wrongCode");
    assert!(desk.entries().is_empty(), "nothing was connected");
  }
}

#[test]
fn startup_join_refuses_an_expired_code() {
  let server = TeamServer::start();
  server.expire_later_code();
  let desk = Desk::new();

  let refusal = refused(desk.run(desk.join.join_by_code(&address(&server), trust(), code(LATER_CODE))));

  // The server answers an expired code as a wrong one, so as not to tell which codes exist.
  assert_eq!(refusal, JoinRefusal::WrongCode);
  assert!(desk.entries().is_empty());
}

#[test]
fn startup_join_refuses_an_ended_invite() {
  let server = TeamServer::start();
  let cloud = cloud_with(&server);
  cloud.invite(INVITE, None);
  let desk = Desk::signed_in(&cloud);
  let invite = desk.run(desk.join.teams()).expect("the account's teams").invites[0].clone();
  // Declined from another of the person's computers meanwhile: the cloud gives no token for it any more.
  desk.run(desk.cloud.decline(invite.id)).expect("decline");

  let refusal = refused(desk.run(desk.join.join_invite(&invite)));

  assert_eq!(refusal, JoinRefusal::InviteEnded);
  assert_eq!(refusal.message().key, "teamJoin.refused.inviteEnded");
  assert_eq!(server.cloud_joins(), 0);
  assert!(desk.entries().is_empty());
}

#[test]
fn startup_join_says_the_server_is_unreachable() {
  let desk = Desk::new();
  let nowhere = closed_address();
  let unreachable = JoinRefusal::Unreachable {
    address: nowhere.clone(),
  };

  assert_eq!(
    refused(desk.run(desk.join.reach(&nowhere, PlainHttp::NotAllowed))),
    unreachable
  );
  assert_eq!(
    refused(desk.run(desk.join.join_by_code(&nowhere, trust(), code(CODE)))),
    unreachable
  );
  let message = unreachable.message();
  assert_eq!(message.key, "teamJoin.refused.unreachable");
  assert_eq!(message.args, [("address", nowhere.as_str().to_owned())]);

  let server = TeamServer::start();
  let team = join_by_code(&desk, &server);
  server.set_down(true);
  assert_eq!(
    refused(desk.run(desk.join.projects(team.id))),
    JoinRefusal::Unreachable {
      address: address(&server)
    }
  );
}

#[test]
fn startup_join_lists_the_projects_after_joining() {
  let server = TeamServer::start();
  server.share(&[
    ("Billing", "billing", Some("github.com/kontur/billing")),
    ("Atlas", "atlas", None),
  ]);
  let desk = Desk::new();
  let team = join_by_code(&desk, &server);

  let projects = desk.run(desk.join.projects(team.id)).expect("the team's projects");

  let listed: Vec<_> = projects
    .iter()
    .map(|listed| {
      (
        listed.project.name.as_str(),
        listed.project.remote_hint.as_ref().map(|hint| hint.as_str()),
      )
    })
    .collect();
  assert_eq!(
    listed,
    [("Billing", Some("github.com/kontur/billing")), ("Atlas", None)]
  );
  assert!(
    projects.iter().all(|listed| listed.checkout.is_none()),
    "no checkout is linked yet"
  );
}

#[test]
fn startup_join_refuses_when_no_project_is_shared() {
  let server = TeamServer::start();
  let desk = Desk::new();
  let team = join_by_code(&desk, &server);

  let refusal = refused(desk.run(desk.join.projects(team.id)));

  assert_eq!(
    refusal,
    JoinRefusal::NoProjects {
      server: NAME.to_owned()
    }
  );
  assert_eq!(refusal.message().args, [("server", NAME.to_owned())]);
}

#[test]
fn startup_join_refusals_have_words() {
  let (_, contents) = LOCALES
    .iter()
    .find(|(locale, _)| *locale == FALLBACK_LOCALE)
    .expect("the fallback locale is shipped");
  let words: Value = serde_json::from_str(contents).expect("the locale is JSON");
  let refusals = [
    JoinRefusal::WrongCode,
    JoinRefusal::InviteEnded,
    JoinRefusal::Unreachable {
      address: ServerAddress::parse("https://orchester.kontur.dev").expect("address"),
    },
    JoinRefusal::NoProjects {
      server: NAME.to_owned(),
    },
  ];
  for refusal in refusals {
    let message = refusal.message();
    let text = message
      .key
      .split('.')
      .fold(&words, |value, part| &value[part])
      .as_str()
      .unwrap_or_else(|| panic!("{} has no words", message.key));
    let mut placeholders: Vec<&str> = text
      .split("{{")
      .skip(1)
      .filter_map(|part| part.split_once("}}").map(|(name, _)| name))
      .collect();
    placeholders.sort_unstable();
    let mut given: Vec<&str> = message.args.iter().map(|(name, _)| *name).collect();
    given.sort_unstable();
    assert_eq!(placeholders, given, "{}", message.key);
  }
}
