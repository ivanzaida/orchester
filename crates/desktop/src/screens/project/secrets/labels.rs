//! How a secret's kind, environment and dates read, shared by the list, the inspector and the
//! dialogs.

use std::sync::Arc;

use chrono::Datelike;
use lurq::app::ctx::Ctx;
use orchester_core::{
  secret::{Environment, SecretKind},
  time::Timestamp,
};

use crate::ui::icon::Icon;

/// The icon a kind of secret is shown with (`hAQ7j` in `vxAMD`, `Z0Ruy` in `CQxCb`).
pub const fn kind_icon(kind: SecretKind) -> Icon {
  match kind {
    SecretKind::AccessToken => Icon::Key,
    SecretKind::ApiKey => Icon::KeyRound,
    SecretKind::DatabaseConnection => Icon::Database,
    SecretKind::DeploymentToken => Icon::Rocket,
    SecretKind::Password => Icon::FormInput,
  }
}

pub const fn kind_key(kind: SecretKind) -> &'static str {
  match kind {
    SecretKind::AccessToken => "secrets.kind.accessToken",
    SecretKind::ApiKey => "secrets.kind.apiKey",
    SecretKind::DatabaseConnection => "secrets.kind.databaseConnection",
    SecretKind::DeploymentToken => "secrets.kind.deploymentToken",
    SecretKind::Password => "secrets.kind.password",
  }
}

pub const fn environment_key(environment: Environment) -> &'static str {
  match environment {
    Environment::Development => "secrets.environment.development",
    Environment::Staging => "secrets.environment.staging",
    Environment::Production => "secrets.environment.production",
  }
}

const MONTHS: [&str; 12] = [
  "time.month.jan",
  "time.month.feb",
  "time.month.mar",
  "time.month.apr",
  "time.month.may",
  "time.month.jun",
  "time.month.jul",
  "time.month.aug",
  "time.month.sep",
  "time.month.oct",
  "time.month.nov",
  "time.month.dec",
];

/// The local calendar date of `at`, e.g. "Sep 20, 2026".
pub fn date(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  let Some(utc) = chrono::DateTime::from_timestamp_millis(at.unix_millis()) else {
    return ctx.t("secrets.inspector.unknownDate");
  };
  let local = utc.with_timezone(&chrono::Local).date_naive();
  let month = ctx.t(MONTHS[local.month0() as usize]);
  ctx.t_args(
    "time.date",
    [
      ("month", &*month),
      ("day", local.day().to_string().as_str()),
      ("year", local.year().to_string().as_str()),
    ],
  )
}
