//! What a tool request is and why it asks, in words: its category (or *Credential read*), the setting of the
//! asking profile it asks under, what the agent explains, and why the classifier asks (`Subject` `F0goVp` and
//! `Reason` `xPiPP` in `gwOaJ`).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  permission::category::{CommandCategory, CommandReason},
  run::SimilarUnavailable,
};
use orchester_workspace::tasks::approvals::{AskingSetting, ShownApproval};

/// What a command is called where its category shows: *Credential read* when it reads a credential
/// location, else its category.
pub(super) const fn subject_key(category: CommandCategory, secrets: bool) -> &'static str {
  if secrets {
    return "cards.approval.category.credentialRead";
  }
  match category {
    CommandCategory::ReadOnly => "cards.approval.category.readOnly",
    CommandCategory::Worktree => "cards.approval.category.worktree",
    CommandCategory::Network => "cards.approval.category.network",
    CommandCategory::OutsideWrite => "cards.approval.category.outsideWrite",
    CommandCategory::Unknown => "cards.approval.category.unknown",
    CommandCategory::HistoryDestructive => "cards.approval.category.historyDestructive",
  }
}

/// What the request asks and what waiting does, by the setting it asks under.
pub(super) fn explanation(
  ctx: &Ctx,
  shown: Option<&ShownApproval>,
  (agent, server, command): (&str, &str, bool),
) -> Arc<str> {
  if !command {
    return ctx.t_args("cards.approval.explain.tool", [("agent", agent), ("server", server)]);
  }
  let key = match shown.and_then(|shown| shown.asking.as_ref()?.setting) {
    Some(AskingSetting::CommandsAsk) => "cards.approval.explain.commandsAsk",
    Some(AskingSetting::NotListed) => "cards.approval.explain.notListed",
    Some(AskingSetting::BeyondSandbox) => "cards.approval.explain.beyondSandbox",
    Some(AskingSetting::McpActions) | None => "cards.approval.explain.command",
  };
  ctx.t_args(key, [("agent", agent)])
}

/// The asking profile's setting at the run's version (*Not an allowed command · Builder v3*).
pub(super) fn policy(ctx: &Ctx, shown: &ShownApproval) -> Option<Arc<str>> {
  let asking = shown.asking.as_ref()?;
  let key = match asking.setting {
    Some(AskingSetting::CommandsAsk) => "cards.approval.policy.commandsAsk",
    Some(AskingSetting::NotListed) => "cards.approval.policy.notListed",
    Some(AskingSetting::BeyondSandbox) => "cards.approval.policy.beyondSandbox",
    Some(AskingSetting::McpActions) => "cards.approval.policy.mcpActions",
    None => "cards.approval.policy.profile",
  };
  let version = asking.version.to_string();
  Some(ctx.t_args(key, [("profile", &*asking.profile), ("version", version.as_str())]))
}

/// Why a command asks, as the classifier read it (*Installs a package from the network: crates.io ·
/// serde*).
pub(super) fn reason(ctx: &Ctx, reason: &CommandReason) -> Arc<str> {
  let (key, value): (&str, Arc<str>) = match reason {
    CommandReason::Unknown => return ctx.t("cards.approval.reason.unknown"),
    // A file that lists the packages names them rather than the first package.
    CommandReason::Installs {
      registry,
      listed_in: Some(file),
      ..
    } => (
      "cards.approval.reason.installsListed",
      ctx.t_args(
        "cards.approval.reason.source",
        [("registry", registry.as_str()), ("package", file.as_str())],
      ),
    ),
    CommandReason::Installs {
      registry,
      package,
      listed_in: None,
    } => (
      "cards.approval.reason.installs",
      match package {
        Some(package) => ctx.t_args(
          "cards.approval.reason.source",
          [("registry", registry.as_str()), ("package", package.as_str())],
        ),
        None => registry.as_str().into(),
      },
    ),
    CommandReason::DeletesOutside { path } => ("cards.approval.reason.deletesOutside", path.as_str().into()),
    CommandReason::RewritesHistory { command } => ("cards.approval.reason.rewritesHistory", command.as_str().into()),
    CommandReason::ReadsCredentials { what } => ("cards.approval.reason.readsCredentials", what.as_str().into()),
    CommandReason::WritesOutside { path } => ("cards.approval.reason.writesOutside", path.as_str().into()),
    CommandReason::Network { command } => ("cards.approval.reason.network", command.as_str().into()),
    CommandReason::Worktree { command } => ("cards.approval.reason.worktree", command.as_str().into()),
    CommandReason::ReadOnly { command } => ("cards.approval.reason.readOnly", command.as_str().into()),
  };
  ctx.t_args(key, [("value", &*value)])
}

/// Why Approve similar is not offered.
pub(super) const fn similar_unavailable(unavailable: SimilarUnavailable) -> &'static str {
  match unavailable {
    SimilarUnavailable::Unknown => "cards.approval.similarUnavailable.unknown",
    SimilarUnavailable::SeveralAsk => "cards.approval.similarUnavailable.severalAsk",
    SimilarUnavailable::Destructive => "cards.approval.similarUnavailable.destructive",
    SimilarUnavailable::Secrets => "cards.approval.similarUnavailable.secrets",
  }
}
