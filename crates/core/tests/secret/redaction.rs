//! A secret value never shows: not in `Debug`, not in an error. Names, kinds and environments
//! are validated and stored by their identifiers.

use std::collections::BTreeMap;

use orchester_core::{
  id::{ProjectId, SecretId},
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, ApprovalMode, CommandAction, DatabaseQuery, DeniedBehavior, HttpAuth, HttpMethod,
    HttpRequest, Tool, ToolAction,
  },
  secret::{
    Environment, SecretError, SecretKind, SecretName, SecretNameError, SecretValue, SecretValueError, VaultError,
    VaultKey, add, tool_uses,
  },
};

use crate::support::{MemoryStore, MemoryVault, StoreFailed, VALUE, at, new_secret, value};

#[test]
fn secret_value_debug_shows_no_content() {
  let secret = value(VALUE);
  let debug = format!("{secret:?} {:?}", Some(&secret));
  assert!(!debug.contains(VALUE));
  assert!(debug.contains("redacted"));
  assert_eq!(secret.expose(), VALUE);
}

#[test]
fn secret_value_must_not_be_blank_and_keeps_surrounding_spaces() {
  for blank in ["", "   ", "\t"] {
    let error = SecretValue::new(blank.to_owned()).expect_err("blank");
    assert_eq!(error, SecretValueError::Blank);
  }
  assert_eq!(value(" fake ").expose(), " fake ");
}

#[test]
fn secret_errors_carry_no_value() {
  let (mut store, vault) = (MemoryStore::default(), MemoryVault::default());
  vault.fail_with(VaultError::AccessDenied("the keychain is locked".to_owned()));
  let error = add(
    &mut store,
    &vault,
    ProjectId::generate(),
    new_secret("STAGING_TOKEN", Environment::Staging),
    &value(VALUE),
    at(1),
  )
  .expect_err("the keychain refuses");
  assert!(matches!(error, SecretError::Vault(VaultError::AccessDenied(_))));
  let shown: String = format!("{error} {error:?}");
  assert!(!shown.contains(VALUE));
  let store_error: SecretError<StoreFailed> = SecretError::ValueLeftBehind {
    source: StoreFailed,
    cleanup: VaultError::Failed("no access".to_owned()),
  };
  assert!(!format!("{store_error} {store_error:?}").contains(VALUE));
}

#[test]
fn secret_name_is_capitals_digits_and_underscores() {
  assert_eq!(
    SecretName::new("  STAGING_DATABASE_URL ").unwrap().as_str(),
    "STAGING_DATABASE_URL"
  );
  assert_eq!(SecretName::new("KEY2").unwrap().as_str(), "KEY2");
  assert_eq!(SecretName::new(" "), Err(SecretNameError::Blank));
  for invalid in ["staging_key", "2FA_KEY", "_KEY", "API KEY", "API-KEY", "КЛЮЧ"] {
    assert_eq!(SecretName::new(invalid), Err(SecretNameError::Invalid), "{invalid}");
  }
  let long = "K".repeat(SecretName::MAX_CHARS + 1);
  assert_eq!(
    SecretName::new(&long),
    Err(SecretNameError::TooLong {
      length: SecretName::MAX_CHARS + 1
    })
  );
}

#[test]
fn secret_kinds_and_environments_round_trip_through_their_identifiers() {
  for kind in SecretKind::ALL {
    assert_eq!(kind.as_str().parse::<SecretKind>(), Ok(kind));
  }
  for environment in Environment::ALL {
    assert_eq!(environment.as_str().parse::<Environment>(), Ok(environment));
  }
  assert!("Staging".parse::<Environment>().is_err());
  assert!("token".parse::<SecretKind>().is_err());
}

#[test]
fn secret_vault_key_names_the_project_and_the_secret() {
  let (project, secret) = (ProjectId::generate(), SecretId::generate());
  let key = VaultKey::new(project, secret);
  assert_eq!(key.service(), format!("orchester:{project}"));
  assert_eq!(key.account(), secret.to_string());
}

fn policy() -> AccessPolicy {
  AccessPolicy::new(
    BTreeMap::new(),
    Name::new("Staging").expect("valid environment"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("valid policy")
}

#[test]
fn secret_is_used_by_the_tools_that_refer_to_it() {
  let (secret, other) = (SecretId::generate(), SecretId::generate());
  let http = |auth| {
    let request = HttpRequest::new(
      HttpMethod::Post,
      "https://deploy.example.test/v1".into(),
      "deploy.example.test".into(),
      auth,
      BTreeMap::new(),
      BTreeMap::new(),
    )
    .expect("valid request");
    Tool::new(ToolAction::Http(request), policy())
  };
  let query = |credentials| {
    let query = DatabaseQuery::new(credentials, "SELECT 1".into()).expect("valid query");
    Tool::new(ToolAction::DatabaseQuery(query), policy())
  };
  let command = Tool::new(
    ToolAction::Command(CommandAction::new("pnpm".into(), vec!["test".into()]).expect("valid command")),
    policy(),
  );
  assert!(tool_uses(&http(HttpAuth::Bearer { secret }), secret));
  assert!(!tool_uses(&http(HttpAuth::Bearer { secret: other }), secret));
  assert!(!tool_uses(&http(HttpAuth::None), secret));
  assert!(tool_uses(&query(secret), secret));
  assert!(!tool_uses(&query(other), secret));
  assert!(!tool_uses(&command, secret));
}
