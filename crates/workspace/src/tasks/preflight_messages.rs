//! What a person reads for each problem that keeps a run from starting ([`PreflightProblem`]).

use orchester_core::{connection::AccountStartProblem, resource::ToolKind, run::PreflightProblem};

use crate::{agents::permissions::hosts_refusal, messages::Message};

/// What a person reads for a preflight problem.
pub fn preflight_message(problem: &PreflightProblem) -> Message {
  let step =
    |key: &'static str, step: &orchester_core::resource::StepKey| Message::new(key).with("step", step.to_string());
  match problem {
    PreflightProblem::PipelineNotPublished => Message::new("runs.preflight.pipelineNotPublished"),
    PreflightProblem::PipelineVersionNotPinned => Message::new("runs.preflight.pipelineVersionNotPinned"),
    PreflightProblem::NoSteps => Message::new("runs.preflight.noSteps"),
    PreflightProblem::ToolNotPublished { step: key } => step("runs.preflight.toolNotPublished", key),
    PreflightProblem::ToolNotRunnable {
      step: key,
      kind: ToolKind::Http,
    } => step("runs.preflight.toolNotRunnable.http", key),
    PreflightProblem::ToolNotRunnable {
      step: key,
      kind: ToolKind::Database,
    } => step("runs.preflight.toolNotRunnable.database", key),
    PreflightProblem::ToolNotRunnable {
      step: key,
      kind: ToolKind::Command,
    } => step("runs.preflight.toolNotRunnable.command", key),
    PreflightProblem::ToolAccessRules { step: key } => step("runs.preflight.toolAccessRules", key),
    PreflightProblem::ApprovalGate { step: key } => step("runs.preflight.approvalGate", key),
    PreflightProblem::ProfileNotPublished { step: key } => step("runs.preflight.profileNotPublished", key),
    PreflightProblem::ConnectionMissing { step: key } => step("runs.preflight.connectionMissing", key),
    PreflightProblem::ConnectionNotPicked { step: key } => step("runs.preflight.connectionNotPicked", key),
    PreflightProblem::McpServerMissing { step: key } => step("runs.preflight.mcpServerMissing", key),
    PreflightProblem::SecretMissing { step: key } => step("runs.preflight.secretMissing", key),
    PreflightProblem::UnsupportedProvider { step: key } => step("runs.preflight.unsupportedProvider", key),
    PreflightProblem::NativeNotAvailable { step: key } => step("runs.preflight.nativeNotAvailable", key),
    PreflightProblem::ConnectionNotUsable { step: key, .. } => step("runs.preflight.connectionNotUsable", key),
    PreflightProblem::AccountUnavailable { step: key, reason } => match reason {
      AccountStartProblem::NoAccount => step("runs.preflight.account.none", key),
      AccountStartProblem::FolderMissing { folder } => {
        step("runs.preflight.account.folderMissing", key).with("folder", folder.display().to_string())
      }
    },
    PreflightProblem::PermissionsNotEnforceable { step: key, .. } => {
      step("runs.preflight.permissionsNotEnforceable", key)
    }
    PreflightProblem::NetworkHostsNotEnforceable { step: key, cli } => step(hosts_refusal(*cli), key),
    PreflightProblem::CommandListNotOffered { step: key } => step("runs.preflight.commandListNotOffered", key),
    PreflightProblem::NotAGitRepository => Message::new("runs.preflight.notAGitRepository"),
    PreflightProblem::NoCommits => Message::new("runs.preflight.noCommits"),
    PreflightProblem::BaseBranchMissing { branch } => {
      Message::new("runs.preflight.baseBranchMissing").with("branch", branch.to_string())
    }
    PreflightProblem::NoBranchCheckedOut { head } => Message::new("runs.preflight.noBranchCheckedOut")
      .with("head", crate::display::short_revision(head.as_str()).to_owned()),
    PreflightProblem::BaseCommitMissing { commit } => Message::new("runs.preflight.baseCommitMissing")
      .with("commit", crate::display::short_revision(commit.as_str()).to_owned()),
  }
}
