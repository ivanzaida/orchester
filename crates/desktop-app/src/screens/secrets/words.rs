//! The words of metadata, reverse references and calendar dates.
use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  id::SecretId,
  secret::{Environment, SecretKind},
  time::Timestamp,
};
use orchester_workspace::{
  display,
  secrets::data::{ProjectSecrets, UseCounts},
};

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
/// How many use a secret, as its page's head says it (`used by 1` in `E7aHKZ`, `06b`).
pub(super) fn used_by(ctx: &Ctx, counts: UseCounts) -> Arc<str> {
  match counts.total() {
    0 => ctx.t("secrets.uses.usedByNone"),
    1 => ctx.t("secrets.uses.usedByOne"),
    count => ctx.t_args("secrets.uses.usedByMany", [("count", count.to_string())]),
  }
}
/// What archiving `secret` does to what uses it, naming each (`1 MCP server uses it: figma. An archived secret gives no
/// value to what uses it.`, `WDDU4` in `06b`).
pub(super) fn archive_impact(ctx: &Ctx, listed: &ProjectSecrets, secret: SecretId) -> Arc<str> {
  let counts = listed.use_counts(secret);
  let key = match (counts.tools, counts.mcp_servers) {
    (0, 0) => "secrets.archiving.unused",
    (1, 0) => "secrets.archiving.tool",
    (_, 0) => "secrets.archiving.tools",
    (0, 1) => "secrets.archiving.server",
    (0, _) => "secrets.archiving.servers",
    _ => "secrets.archiving.mixed",
  };
  let names: Vec<&str> = listed.used_by(secret).map(|used| used.name.as_ref()).collect();
  ctx.t_args(
    key,
    [("count", counts.total().to_string()), ("names", names.join(", "))],
  )
}
/// The day `at` as the list says it (`set Oct 24`, `set today` in `06` `bVh1e`): *today*, the month and day in this
/// year, or with the year before it.
pub(super) fn date(ctx: &mut Ctx, at: Timestamp) -> Arc<str> {
  let (month, day, year) = display::calendar_date(at);
  let now = display::calendar_date(crate::services::Services::of(ctx).clock.now());
  if (month, day, year) == now {
    return ctx.t("secrets.today");
  }
  let month = ctx.t_ns(orchester_workspace::i18n::NAMESPACE, display::MONTH_KEYS[month]);
  if year == now.2 {
    return ctx.t_args(
      "secrets.dateThisYear",
      [("month", month.to_string()), ("day", day.to_string())],
    );
  }
  ctx.t_args(
    "secrets.date",
    [
      ("month", month.to_string()),
      ("day", day.to_string()),
      ("year", year.to_string()),
    ],
  )
}
