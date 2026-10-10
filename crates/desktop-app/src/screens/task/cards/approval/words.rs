//! What a tool request is and why it asks, in words: its category (or *Credential read*), the setting of the
//! asking profile it asks under, what the agent explains, and why the classifier asks (`Subject` `F0goVp` and
//! `Reason` `xPiPP` in `gwOaJ`).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  orchester_tools::{self, OrchesterTool},
  permission::category::{CommandCategory, CommandReason},
  run::{PendingToolApproval, SimilarUnavailable},
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

/// What made one of Orchester's own tools ask: it reads outside the worktree, in a folder the run's sources
/// do not hold, or it runs a check. Other MCP calls ask under the profile's setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OwnReach<'a> {
  Outside(&'a str),
  Check(&'a str),
}

/// What made `request` ask, when it is a call of one of Orchester's own tools that reaches outside the
/// worktree, as core reads it.
pub(super) fn own_reach(request: &PendingToolApproval) -> Option<OwnReach<'_>> {
  if request.command || request.server != orchester_tools::SERVER {
    return None;
  }
  let tool = OrchesterTool::from_name(&request.action)?;
  match (tool, request.target.as_deref()) {
    (tool, Some(folder)) if tool.reads_outside() => Some(OwnReach::Outside(folder)),
    (OrchesterTool::RunCheck, Some(name)) => Some(OwnReach::Check(name)),
    _ => None,
  }
}

/// Why one of Orchester's own tools asks, the folder or check named (*Reads outside the worktree:
/// H:\projects\kontur-docs*).
pub(super) fn own_reason(ctx: &Ctx, reach: OwnReach<'_>) -> Arc<str> {
  match reach {
    OwnReach::Outside(folder) => ctx.t_args(
      "cards.approval.reason.readsOutside",
      [("value", platform_path(folder).as_str())],
    ),
    OwnReach::Check(name) => ctx.t_args("cards.approval.reason.runsCheck", [("value", name)]),
  }
}

/// `folder` with the platform's separator between its parts, however it was recorded (an agent may mix `/`
/// into a Windows path).
pub(super) fn platform_path(folder: &str) -> String {
  std::path::Path::new(folder)
    .components()
    .collect::<std::path::PathBuf>()
    .display()
    .to_string()
}

/// What the request asks and what waiting does, by the setting it asks under, or by what one of Orchester's
/// own tools reaches.
pub(super) fn explanation(ctx: &Ctx, shown: Option<&ShownApproval>, request: &PendingToolApproval) -> Arc<str> {
  let agent = request.agent.as_str();
  match own_reach(request) {
    Some(OwnReach::Outside(_)) => return ctx.t_args("cards.approval.explain.readsOutside", [("agent", agent)]),
    Some(OwnReach::Check(_)) => return ctx.t_args("cards.approval.explain.runsCheck", [("agent", agent)]),
    None => {}
  }
  if !request.command {
    return ctx.t_args(
      "cards.approval.explain.tool",
      [("agent", agent), ("server", request.server.as_str())],
    );
  }
  let key = match shown.and_then(|shown| shown.asking.as_ref()?.setting) {
    Some(AskingSetting::CommandsAsk) => "cards.approval.explain.commandsAsk",
    Some(AskingSetting::NotListed) => "cards.approval.explain.notListed",
    Some(AskingSetting::BeyondSandbox) => "cards.approval.explain.beyondSandbox",
    Some(AskingSetting::McpActions) | None => "cards.approval.explain.command",
  };
  ctx.t_args(key, [("agent", agent)])
}

/// The asking profile's setting at the run's version (*Not an allowed command · Builder v3*); for one of
/// Orchester's own tools, what it reaches, which is what made it ask.
pub(super) fn policy(ctx: &Ctx, shown: &ShownApproval) -> Option<Arc<str>> {
  let asking = shown.asking.as_ref()?;
  let key = match (own_reach(&shown.approval), asking.setting) {
    (Some(OwnReach::Outside(_)), _) => "cards.approval.policy.readsOutside",
    (Some(OwnReach::Check(_)), _) => "cards.approval.policy.runsCheck",
    (None, setting) => match setting {
      Some(AskingSetting::CommandsAsk) => "cards.approval.policy.commandsAsk",
      Some(AskingSetting::NotListed) => "cards.approval.policy.notListed",
      Some(AskingSetting::BeyondSandbox) => "cards.approval.policy.beyondSandbox",
      Some(AskingSetting::McpActions) => "cards.approval.policy.mcpActions",
      None => "cards.approval.policy.profile",
    },
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
    SimilarUnavailable::BroadFolder => "cards.approval.similarUnavailable.broadFolder",
    SimilarUnavailable::EveryCall => "cards.approval.similarUnavailable.everyCall",
  }
}
