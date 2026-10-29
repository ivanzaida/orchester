//! How an agent CLI starts per approval mode (`docs/design/approval-modes.md` › Enforcement): Autopilot turns the
//! CLI's own permission prompt off, so that every tool request reaches Orchester, and a Manual step starts as before;
//! a CLI that cannot hand its requests to Orchester runs Autopilot as Manual with the reason.

use std::path::Path;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
    claude_approval::{ClaudeApprovalScope, ClaudeAsk, PROMPT_TOOL},
    invocation::{ClaudeInvocation, ClaudeSession, Platform},
    mcp::{InvocationExemptions, McpServerName},
  },
  id::ConnectionId,
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, ApprovalMode, AutopilotMechanism, AutopilotSupport, CLAUDE_CODE_AUTOPILOT,
    CODEX_AUTOPILOT, CantEnforce, CommandPermission, CommandPrefix, EffectiveApprovalMode, ManualReason, NetworkAccess,
    RepositoryAccess, autopilot_support,
    capability::{EnforcedPermissions, Mechanism, PermissionOption, PermissionProblem, PermissionSetting, Unavailable},
    claude::{PermissionFolders, permission_args},
  },
  time::Timestamp,
};
use serde_json::json;

const PLATFORMS: [Platform; 2] = [Platform::Windows, Platform::MacOs];
const ASK_EVERY_COMMAND: &str = r#"{"permissions":{"ask":["Bash"]}}"#;

fn git_only() -> CommandPermission {
  CommandPermission::Allowed(
    AllowedCommands::new(vec![CommandPrefix::new("git").expect("valid prefix")]).expect("valid list"),
  )
}

fn permissions(commands: CommandPermission, mode: ApprovalMode) -> AgentPermissions {
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::On);
  permissions.set_approval_mode(mode);
  permissions
}

fn docs() -> McpServerName {
  McpServerName::new("docs").expect("valid server name")
}

fn claude_args(enforced: &EnforcedPermissions) -> Vec<String> {
  let worktree = Path::new("/repo/.orchester/worktrees/run");
  let folders = PermissionFolders {
    editable: &[worktree],
    protected: &[],
  };
  permission_args(enforced, &[docs()], &InvocationExemptions::default(), folders)
}

/// The values of `flag` in `args`, up to the next option.
fn values<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
  args
    .iter()
    .skip_while(|arg| *arg != flag)
    .skip(1)
    .take_while(|arg| !arg.starts_with("--"))
    .map(String::as_str)
    .collect()
}

#[test]
fn approval_mode_launch_claude_autopilot_gives_no_rule_that_runs_a_request_by_itself() {
  for platform in PLATFORMS {
    let manual = permissions(git_only(), ApprovalMode::Manual)
      .enforced_on(AgentProvider::ClaudeCodeCli, platform)
      .expect("Manual is enforceable");
    let manual = claude_args(&manual);
    // Manual starts as before: the listed commands and the servers' tools run by rule, edits by rule.
    assert_eq!(values(&manual, "--permission-mode"), ["default"]);
    assert!(values(&manual, "--allowedTools").contains(&"Bash(git:*)"), "{manual:?}");
    assert!(values(&manual, "--allowedTools").contains(&"mcp__docs"), "{manual:?}");
    assert!(!manual.iter().any(|arg| arg == "--settings"), "{manual:?}");

    let autopilot = permissions(git_only(), ApprovalMode::Autopilot)
      .enforced_on(AgentProvider::ClaudeCodeCli, platform)
      .expect("Autopilot is enforceable");
    assert_eq!(
      autopilot.mechanism(PermissionSetting::Commands),
      Mechanism::ClaudePermissionPrompt
    );
    let autopilot = claude_args(&autopilot);
    // Every command asks, so the read-only classifier and `acceptEdits` decide none; no command or MCP rule.
    assert_eq!(values(&autopilot, "--settings"), [ASK_EVERY_COMMAND]);
    assert_eq!(values(&autopilot, "--permission-mode"), ["acceptEdits"]);
    assert!(!autopilot.iter().any(|arg| arg == "--allowedTools"), "{autopilot:?}");
    assert!(!autopilot.iter().any(|arg| arg.starts_with("Bash(")), "{autopilot:?}");
  }
}

#[test]
fn approval_mode_launch_claude_autopilot_refuses_what_its_manual_fallback_cannot_run() {
  // *Any command in the worktree*, which Claude Code cannot confine, is refused in Autopilot too: where the CLI is
  // too old or never checked, the step runs as Manual, which would run it.
  for platform in PLATFORMS {
    let autopilot = permissions(CommandPermission::AnyInWorktree, ApprovalMode::Autopilot);
    assert_eq!(
      autopilot.enforced_on(AgentProvider::ClaudeCodeCli, platform),
      Err(PermissionProblem::NotEnforceable {
        option: PermissionOption::AnyCommand,
        reason: Unavailable::CannotConfineCommands,
      })
    );
    let too_old = EffectiveApprovalMode::of(
      autopilot.approval_mode(),
      autopilot_support(AgentProvider::ClaudeCodeCli, Some("2.1.88")),
    );
    let unchecked = EffectiveApprovalMode::of(
      autopilot.approval_mode(),
      autopilot_support(AgentProvider::ClaudeCodeCli, None),
    );
    for effective in [too_old, unchecked] {
      assert!(matches!(
        effective,
        EffectiveApprovalMode::Manual {
          why: ManualReason::CantEnforce { .. }
        }
      ));
      assert!(
        autopilot
          .clone()
          .with_effective_mode(&effective)
          .enforced_on(AgentProvider::ClaudeCodeCli, platform)
          .is_err()
      );
    }
    // Codex confines any command, so it runs Autopilot and its Manual fallback alike.
    assert!(autopilot.enforced_on(AgentProvider::CodexCli, platform).is_ok());
  }
}

#[test]
fn approval_mode_launch_claude_autopilot_hands_every_command_and_mcp_call_to_the_prompt() {
  // Even a listed command, which Claude Code would run by rule in Manual, reaches Orchester in Autopilot.
  let autopilot = permissions(git_only(), ApprovalMode::Autopilot);
  let enforced = autopilot
    .enforced_on(AgentProvider::ClaudeCodeCli, Platform::current())
    .expect("Autopilot hands commands to Orchester");
  let scope = ClaudeApprovalScope::of(enforced.permissions(), &[docs()]).expect("Autopilot uses the prompt");
  let call =
    |tool: &str, input: serde_json::Value| json!({"tool_name": tool, "input": input, "tool_use_id": "toolu_1"});
  for (tool, input) in [
    ("Bash", json!({"command": "git status --short"})),
    ("mcp__docs__search", json!({"query": "auth"})),
  ] {
    assert!(
      matches!(scope.classify(&call(tool, input)), ClaudeAsk::Person(_)),
      "{tool} reaches Orchester"
    );
  }
  // The invocation starts with the prompt tool and its relay, new session or resumed.
  for session in [ClaudeSession::New(uuid::Uuid::nil()), ClaudeSession::Resume("s-1")] {
    let invocation = ClaudeInvocation {
      executable: Path::new("/bin/claude"),
      model: "haiku",
      effort: None,
      permissions: &enforced,
      writable_roots: &[],
      worktree: Path::new("/repo/.orchester/worktrees/run"),
      session,
      mcp: None,
      skills: None,
      code_index: None,
      approvals: Some(Path::new("/tmp/approvals.json")),
      checks: &[],
    };
    let args: Vec<String> = invocation
      .args()
      .into_iter()
      .map(|arg| arg.into_string().expect("unicode"))
      .collect();
    assert_eq!(values(&args, "--permission-prompt-tool"), [PROMPT_TOOL]);
    assert_eq!(values(&args, "--settings"), [ASK_EVERY_COMMAND]);
  }
}

#[test]
fn approval_mode_launch_manual_keeps_the_scope_it_had() {
  let manual = permissions(CommandPermission::AnyInWorktree, ApprovalMode::Manual);
  assert!(ClaudeApprovalScope::of(&manual, &[docs()]).is_none());
  let enforced = permissions(CommandPermission::AnyInWorktree, ApprovalMode::Manual)
    .enforced_on(AgentProvider::CodexCli, Platform::current())
    .expect("Codex confines commands");
  assert_eq!(
    enforced.mechanism(PermissionSetting::Commands),
    Mechanism::CodexSandboxedCommands
  );
  let autopilot = permissions(CommandPermission::AnyInWorktree, ApprovalMode::Autopilot)
    .enforced_on(AgentProvider::CodexCli, Platform::current())
    .expect("Codex hands its requests to Orchester");
  assert_eq!(
    autopilot.mechanism(PermissionSetting::Commands),
    Mechanism::CodexCommandApprovals
  );
}

#[test]
fn approval_mode_launch_support_per_cli_and_version() {
  use AgentProvider::{AnthropicApi, ClaudeCodeCli, CodexCli, OpenAiApi};
  let enforced = AutopilotSupport::Enforced;
  assert_eq!(
    autopilot_support(ClaudeCodeCli, Some(CLAUDE_CODE_AUTOPILOT)),
    enforced(AutopilotMechanism::ClaudePermissionPrompt)
  );
  assert_eq!(
    autopilot_support(ClaudeCodeCli, Some("2.1.301")),
    enforced(AutopilotMechanism::ClaudePermissionPrompt)
  );
  assert_eq!(
    autopilot_support(ClaudeCodeCli, Some("2.1.88")),
    AutopilotSupport::CantEnforce(CantEnforce::CliTooOld {
      installed: "2.1.88".to_owned(),
      required: CLAUDE_CODE_AUTOPILOT.to_owned(),
    })
  );
  assert_eq!(
    autopilot_support(CodexCli, Some("0.159.2")),
    enforced(AutopilotMechanism::CodexAppServer)
  );
  assert_eq!(
    autopilot_support(CodexCli, Some("0.139.0")),
    AutopilotSupport::CantEnforce(CantEnforce::CliTooOld {
      installed: "0.139.0".to_owned(),
      required: CODEX_AUTOPILOT.to_owned(),
    })
  );
  assert_eq!(
    autopilot_support(CodexCli, None),
    AutopilotSupport::CantEnforce(CantEnforce::VersionUnknown {
      required: CODEX_AUTOPILOT.to_owned(),
    })
  );
  for native in [AnthropicApi, OpenAiApi] {
    assert_eq!(autopilot_support(native, None), enforced(AutopilotMechanism::Orchester));
  }
}

#[test]
fn approval_mode_launch_cant_enforce_falls_back_to_manual_with_the_reason() {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex · Kontur").expect("name"),
    AgentProvider::CodexCli,
    std::env::temp_dir().join("codex"),
    Timestamp::now(),
  )
  .expect("absolute path");
  let unknown = AutopilotSupport::CantEnforce(CantEnforce::VersionUnknown {
    required: CODEX_AUTOPILOT.to_owned(),
  });
  assert_eq!(connection.autopilot_support(), unknown);
  let check = |version: &str| ConnectionCheck {
    at: Timestamp::now(),
    result: CheckResult::Available {
      version: version.to_owned(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  };
  connection.record_check(check("0.139.0")).expect("a CLI check");
  let profile = permissions(CommandPermission::AnyInWorktree, ApprovalMode::Autopilot);

  let effective = EffectiveApprovalMode::of(profile.approval_mode(), connection.autopilot_support());
  let too_old = CantEnforce::CliTooOld {
    installed: "0.139.0".to_owned(),
    required: CODEX_AUTOPILOT.to_owned(),
  };
  assert_eq!(
    effective,
    EffectiveApprovalMode::Manual {
      why: ManualReason::CantEnforce { why: too_old }
    }
  );
  let ran = profile.clone().with_effective_mode(&effective);
  assert_eq!(ran.approval_mode(), ApprovalMode::Manual);
  assert!(!ran.hands_requests_to_orchester());
  // Manual as it was: everything but the mode is the profile's.
  let mut expected = profile.clone();
  expected.set_approval_mode(ApprovalMode::Manual);
  assert_eq!(ran, expected);
  assert_eq!(
    serde_json::to_value(&effective).expect("serializes"),
    json!({"mode": "manual", "why": {"reason": "cant_enforce",
      "why": {"reason": "cli_too_old", "installed": "0.139.0", "required": CODEX_AUTOPILOT}}})
  );

  connection.record_check(check("0.159.2")).expect("a CLI check");
  let effective = EffectiveApprovalMode::of(profile.approval_mode(), connection.autopilot_support());
  assert_eq!(
    effective,
    EffectiveApprovalMode::Autopilot {
      how: AutopilotMechanism::CodexAppServer
    }
  );
  assert!(profile.with_effective_mode(&effective).hands_requests_to_orchester());

  // A Manual profile stays Manual whatever its CLI can do.
  let manual = permissions(CommandPermission::AnyInWorktree, ApprovalMode::Manual);
  assert_eq!(
    EffectiveApprovalMode::of(manual.approval_mode(), connection.autopilot_support()),
    EffectiveApprovalMode::Manual {
      why: ManualReason::Profile
    }
  );
}
