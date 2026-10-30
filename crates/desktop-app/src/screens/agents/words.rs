//! What the Profiles tab calls things: a version's state, the permissions in a few words, an effort and a
//! connection's CLI and account.
use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::{AgentConnection, AgentProvider, invocation::Platform},
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::RepositoryAccess,
};
use orchester_workspace::agents::data::VersionState;

use crate::services::accounts::Accounts;

/// A version's state in a list row: `Published v4`, `v2 · Draft v3 saved`, `Draft v1 · not published`.
pub fn list_version(ctx: &Ctx, state: VersionState) -> Arc<str> {
  match state {
    VersionState::Draft { published: None } => ctx.t("agents.firstDraft"),
    VersionState::Draft {
      published: Some(version),
    } => ctx.t_args(
      "agents.draft",
      [
        ("version", version.get().to_string()),
        ("next", version.get().saturating_add(1).to_string()),
      ],
    ),
    VersionState::StaleDraft { based_on, published } => ctx.t_args(
      "agents.stale",
      [
        ("base", based_on.map_or(0, |v| v.get()).to_string()),
        ("version", published.map_or(0, |v| v.get()).to_string()),
      ],
    ),
    VersionState::Published(version) => ctx.t_args("agents.published", [("version", version.get().to_string())]),
  }
}

/// An effort as the app words it: Low, Medium and High in words, the CLI's own levels as it takes them.
pub fn effort(ctx: &Ctx, effort: &str) -> Arc<str> {
  match effort {
    "low" => ctx.t("agents.effortLow"),
    "medium" => ctx.t("agents.effortMedium"),
    "high" => ctx.t("agents.effortHigh"),
    other => other.into(),
  }
}

pub fn commands(ctx: &Ctx, permissions: &AgentPermissions) -> Arc<str> {
  match permissions.commands() {
    CommandPermission::AskBefore => ctx.t("agents.asksCommands"),
    CommandPermission::AnyInWorktree => ctx.t("agents.anyCommands"),
    CommandPermission::Allowed(list) => {
      ctx.t_args("agents.commandsCount", [("count", list.entries().len().to_string())])
    }
  }
}

pub fn network(ctx: &Ctx, permissions: &AgentPermissions, provider: Option<AgentProvider>) -> Arc<str> {
  let Some(provider) = provider else {
    return ctx.t("agents.networkUnknown");
  };
  let effective = permissions.effective_network(provider, Platform::current());
  ctx.t(if effective != *permissions.network() {
    "agents.networkFallback"
  } else if effective == NetworkAccess::On {
    "agents.networkOn"
  } else {
    "agents.networkOff"
  })
}

/// `Edits files · 2 allowed commands · Network on`.
pub fn summary(ctx: &Ctx, permissions: &AgentPermissions, provider: Option<AgentProvider>) -> Arc<str> {
  let repository = ctx.t(match permissions.repository() {
    RepositoryAccess::ReadOnly => "agents.readOnly",
    RepositoryAccess::Write => "agents.write",
  });
  ctx.t_args(
    "agents.summary",
    [
      ("repository", repository),
      ("commands", commands(ctx, permissions)),
      ("network", network(ctx, permissions, provider)),
    ],
  )
}

/// What a connection's choice says under its name (`M42rM`): its CLI and account with who is signed in there and the
/// week's use, or, in `warning`, what keeps runs from starting on it.
pub fn connection_choice(
  ctx: &Ctx,
  connection: &AgentConnection,
  (accounts, readings): (&Accounts, &orchester_core::connection::plan_limits::PlanReadings),
) -> (String, bool) {
  use crate::services::accounts::{DetailPlace, account_name, identity_detail, short_state};
  if let Some(provider) = connection.api_provider() {
    let text = ctx.t_args("agents.connectionApiProvider", [("host", provider.base_url().host())]);
    return (text.to_string(), false);
  }
  let (Some(cli), Some(account)) = (
    orchester_core::account::AccountCli::of(connection.provider()),
    accounts.of_connection(connection.account(), connection.provider()),
  ) else {
    // A native connection runs on no account of the person's: its name says it.
    return (String::new(), false);
  };
  let cli = ctx
    .t(match cli {
      orchester_core::account::AccountCli::ClaudeCode => "accounts.cli.claudeCode",
      orchester_core::account::AccountCli::Codex => "accounts.cli.codex",
    })
    .to_string();
  let name = account_name(ctx, account);
  if let Some(problem) = short_state(ctx, account) {
    let text = ctx.t_args(
      "agents.connectionProblem",
      [("cli", cli), ("account", name), ("problem", problem)],
    );
    return (text.to_string(), true);
  }
  let detail = identity_detail(
    ctx,
    (account, accounts),
    readings.of_account(account.id()),
    DetailPlace::AfterComma,
  );
  let text = ctx.t_args(
    "agents.connectionDetail",
    [("cli", cli), ("account", name), ("detail", detail)],
  );
  (text.to_string(), false)
}
