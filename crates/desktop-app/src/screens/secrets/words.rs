//! The words of metadata, reverse references and calendar dates.
use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  secret::{Environment, SecretKind},
  time::Timestamp,
};
use orchester_workspace::{display, secrets::data::UseCounts};

use crate::ui::icon::Icon;

pub(super) const fn kind_key(kind: SecretKind) -> &'static str {
  match kind {
    SecretKind::AccessToken => "secrets.kinds.access_token",
    SecretKind::ApiKey => "secrets.kinds.api_key",
    SecretKind::DatabaseConnection => "secrets.kinds.database_connection",
    SecretKind::DeploymentToken => "secrets.kinds.deployment_token",
    SecretKind::Password => "secrets.kinds.password",
  }
}
pub(super) const fn environment_key(environment: Environment) -> &'static str {
  match environment {
    Environment::Development => "secrets.environments.development",
    Environment::Staging => "secrets.environments.staging",
    Environment::Production => "secrets.environments.production",
  }
}
pub(super) const fn glyph(kind: SecretKind) -> Icon {
  match kind {
    SecretKind::AccessToken => Icon::Key,
    SecretKind::ApiKey => Icon::KeyRound,
    SecretKind::DatabaseConnection => Icon::Database,
    SecretKind::DeploymentToken => Icon::Rocket,
    SecretKind::Password => Icon::RectangleEllipsis,
  }
}
pub(super) fn uses(ctx: &Ctx, counts: UseCounts) -> Arc<str> {
  let key = match (counts.tools, counts.mcp_servers) {
    (0, 0) => "secrets.uses.unused",
    (1, 0) => "secrets.uses.tool",
    (_, 0) => "secrets.uses.tools",
    (0, 1) => "secrets.uses.server",
    (0, _) => "secrets.uses.servers",
    _ => "secrets.uses.resources",
  };
  ctx.t_args(key, [("count", counts.total().to_string())])
}
pub(super) fn archive_impact(ctx: &Ctx, counts: UseCounts) -> Arc<str> {
  let key = match (counts.tools, counts.mcp_servers) {
    (0, 0) => "secrets.archiving.unused",
    (1, 0) => "secrets.archiving.tool",
    (_, 0) => "secrets.archiving.tools",
    (0, 1) => "secrets.archiving.server",
    (0, _) => "secrets.archiving.servers",
    _ => "secrets.archiving.mixed",
  };
  ctx.t_args(key, [("count", counts.total().to_string())])
}
pub(super) fn date(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  let (month, day, year) = display::calendar_date(at);
  let month = ctx.t_ns(orchester_workspace::i18n::NAMESPACE, display::MONTH_KEYS[month]);
  ctx.t_args(
    "secrets.date",
    [
      ("month", month.to_string()),
      ("day", day.to_string()),
      ("year", year.to_string()),
    ],
  )
}
