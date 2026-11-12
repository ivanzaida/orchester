//! `local-hub-tools`: the hub of `local-hub` with the project's secrets `GITHUB_TOKEN` and `SENTRY_TOKEN` (Production)
//! and `ORDERS_DB` (Staging), and two tools that use them (`Agents · v2` L20–L24): `sentry-issues`, an HTTP request
//! that sends `SENTRY_TOKEN` as its bearer token, and `orders-report`, a database query that connects with `ORDERS_DB`,
//! at v2. *Agents › Tools › New tool* makes either kind (L20, L22, L23); each tool's panel edits it in place (L21,
//! L24).

use std::collections::BTreeMap;

use orchester_core::{
  id::SecretId,
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, ApprovalMode, DatabaseQuery, DeniedBehavior, FieldName, HttpAuth, HttpMethod,
    HttpRequest, RequestTarget, ResponseSource, Tool, ToolAction,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::SecretStore,
};

use super::agents_library::publish;
use crate::kontur::{Kontur, SeedError};

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::local_hub::seed(kontur)?;
  secret(
    kontur,
    "GITHUB_TOKEN",
    (SecretKind::AccessToken, Environment::Production),
  )?;
  let sentry = secret(
    kontur,
    "SENTRY_TOKEN",
    (SecretKind::AccessToken, Environment::Production),
  )?;
  let orders = secret(
    kontur,
    "ORDERS_DB",
    (SecretKind::DatabaseConnection, Environment::Staging),
  )?;
  publish(kontur, "sentry-issues", vec![sentry_issues(sentry)?])?;
  let report = |query: &str| orders_report(orders, query);
  publish(
    kontur,
    "orders-report",
    vec![
      report("select id, status, total\nfrom orders\nwhere created_at > now() - interval '1 day'")?,
      report(
        "select id, status, total\nfrom orders\nwhere created_at > now() - interval '1 day'\norder by created_at desc",
      )?,
    ],
  )
  .map(drop)
}

/// The project's secret `name`, with no value kept for it.
fn secret(
  kontur: &mut Kontur,
  name: &str,
  (kind, environment): (SecretKind, Environment),
) -> Result<SecretId, SeedError> {
  let secret = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new(name).map_err(invalid)?,
      kind,
      environment,
    },
    kontur.ago(24 * 60),
  );
  kontur.db.create_secret(&secret)?;
  Ok(secret.id())
}

fn policy(environment: &str, approval: ApprovalMode, window: AccessWindow) -> Result<AccessPolicy, SeedError> {
  AccessPolicy::new(
    BTreeMap::new(),
    Name::new(environment).map_err(invalid)?,
    approval,
    window,
    DeniedBehavior::AwaitChangedPlan,
  )
  .map_err(invalid)
}

/// Finds the project's Sentry issues that match a search, sending `token` as its bearer token.
fn sentry_issues(token: SecretId) -> Result<Tool, SeedError> {
  let field = |name: &str| FieldName::new(name).map_err(invalid);
  let request = HttpRequest::new(
    HttpMethod::Get,
    "https://sentry.io/api/0/projects/acme/kontur/issues/".into(),
    "sentry.io".into(),
    HttpAuth::Bearer { secret: token },
    BTreeMap::from([
      (field("search")?, RequestTarget::Query("query".into())),
      (field("limit")?, RequestTarget::Query("per_page".into())),
    ]),
    BTreeMap::from([(field("status")?, ResponseSource::Status)]),
  )
  .map_err(invalid)?;
  let access = policy("Production", ApprovalMode::Never, AccessWindow::UntilStepEnds)?;
  Tool::new(
    "Finds the project's Sentry issues that match a search",
    ToolAction::Http(request),
    Some(access),
  )
  .map_err(invalid)
}

/// Lists yesterday's orders with `query`, connecting with `credentials`.
fn orders_report(credentials: SecretId, query: &str) -> Result<Tool, SeedError> {
  let query = DatabaseQuery::new(credentials, query.into()).map_err(invalid)?;
  let minutes = AccessWindow::Minutes(15.try_into().map_err(invalid)?);
  let access = policy("Staging", ApprovalMode::OncePerRun, minutes)?;
  Tool::new(
    "Lists yesterday's orders with their status and total",
    ToolAction::DatabaseQuery(query),
    Some(access),
  )
  .map_err(invalid)
}
