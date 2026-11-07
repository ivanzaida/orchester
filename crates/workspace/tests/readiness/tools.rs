//! A tool that sends or connects with a project secret is ready only when that secret has a value.

use orchester_core::{
  id::SecretId,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::Missing,
};
use orchester_workspace::readiness::{Group, ReadinessStanding, Undecided};

use crate::support::{Fixture, http_tool, secret};

#[test]
fn readiness_a_local_tool_whose_secret_has_no_value_is_missing_it() {
  let fixture = Fixture::local();
  let token = fixture.project_secret("DEPLOY_TOKEN", None);
  let tool = fixture.own("deploy", &http_tool(token));

  let read = fixture.read();
  let found = read.resource(tool).expect("listed");
  assert_eq!(found.group(), Group::NeedsYouHere);
  assert_eq!(
    found.standing().missing(),
    [Missing::LocalSecret {
      secret: token,
      name: secret("DEPLOY_TOKEN")
    }]
  );
}

#[test]
fn readiness_a_local_tool_is_ready_when_its_secret_has_a_value_in_the_keychain() {
  let fixture = Fixture::local();
  let token = fixture.project_secret("DEPLOY_TOKEN", Some("s3cret-value"));
  let tool = fixture.own("deploy", &http_tool(token));

  let read = fixture.read();
  assert_eq!(
    read.resource(tool).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
  assert!(
    !format!("{read:?}").contains("s3cret-value"),
    "the value never reaches the answer"
  );
}

#[test]
fn readiness_another_secret_of_the_same_name_does_not_stand_in() {
  let fixture = Fixture::local();
  let other = fixture.project_secret("DEPLOY_TOKEN", Some("a"));
  let unset = SecretId::generate();
  let tool = fixture.own("deploy", &http_tool(unset));

  assert_ne!(other, unset);
  assert_eq!(
    fixture.read().resource(tool).expect("listed").standing(),
    &ReadinessStanding::Undecided(Undecided::SecretGone(unset)),
    "a tool that names a secret the project does not have is not ready"
  );
}

#[test]
fn readiness_a_keychain_that_cannot_say_leaves_the_tool_undecided() {
  struct Locked;
  impl SecretVault for Locked {
    fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
      Err(VaultError::AccessDenied("locked".to_owned()))
    }

    fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
      Err(VaultError::AccessDenied("locked".to_owned()))
    }

    fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
      Ok(())
    }
  }

  let fixture = Fixture::local();
  let token = fixture.project_secret("DEPLOY_TOKEN", Some("a"));
  let tool = fixture.own("deploy", &http_tool(token));

  let read = fixture
    .workspace
    .project_readiness(&fixture.root, &Locked)
    .expect("readiness");
  assert_eq!(
    read.resource(tool).expect("listed").standing(),
    &ReadinessStanding::Undecided(Undecided::SecretUnreadable(token))
  );
}

#[test]
fn readiness_a_team_tool_holding_a_secret_by_id_is_undecided_not_ready() {
  let fixture = Fixture::team();
  let token = SecretId::generate();
  let tool = fixture.team_resource("deploy", http_tool(token));

  assert_eq!(
    fixture.read().resource(tool).expect("listed").standing(),
    &ReadinessStanding::Undecided(Undecided::TeamToolSecretById(token))
  );
}
