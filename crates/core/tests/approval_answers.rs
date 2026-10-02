use std::str::FromStr;

use orchester_core::{
  connection::mcp::{ExemptTools, McpToolName},
  id::{InvocationId, RunId, StepExecutionId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{CommandCategory, CommandScope},
  },
  run::{
    AlwaysAllowError, AlwaysUnavailable, ApprovalAnswer, ApprovalRule, ApprovalRuleId, ApprovalSubject, AutoAsk,
    AutoVerdict, DecidedBy, PendingToolApproval, RuleScope, SimilarUnavailable, allowing_command, allowing_tool,
    always_prefixes, answering, auto_approves, auto_verdict, denial_message,
  },
  time::Timestamp,
};
use tempfile::TempDir;

struct Fixture {
  _root: TempDir,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = home.join("kontur");
    let temp = root.path().join("temp");
    for folder in [&worktree, &temp] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    let scope = CommandScope::new(&worktree, &[], Some(&home), Some(&temp));
    Self { _root: root, scope }
  }

  fn command(&self, command: &str) -> ApprovalSubject {
    ApprovalSubject::of(&approval(true, "command", command), &self.scope)
  }
}

fn mcp(server: &str, tool: &str) -> ApprovalSubject {
  ApprovalSubject::McpTool {
    server: server.to_owned(),
    tool: tool.to_owned(),
  }
}

fn approval(command: bool, server: &str, action: &str) -> PendingToolApproval {
  PendingToolApproval {
    id: "invocation:1".to_owned(),
    run: RunId::generate(),
    execution: StepExecutionId::generate(),
    invocation: InvocationId::generate(),
    agent: "Builder".to_owned(),
    command,
    scope: None,
    server: server.to_owned(),
    action: action.to_owned(),
    target: None,
    requested_at: Timestamp::from_unix_millis(0),
    native: None,
  }
}

fn rule(id: i64, scope: RuleScope) -> ApprovalRule {
  ApprovalRule {
    id: ApprovalRuleId::new(id),
    run: RunId::generate(),
    scope,
    created_at: Timestamp::from_unix_millis(0),
    revoked_at: None,
    answered: 0,
  }
}

fn listing(prefixes: &[&str]) -> AgentPermissions {
  let entries = prefixes
    .iter()
    .map(|prefix| CommandPrefix::new(prefix).expect("prefix"))
    .collect();
  AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(AllowedCommands::new(entries).expect("list")),
    NetworkAccess::On,
  )
}

#[test]
fn approval_answers_similar_command_is_program_subcommand_and_category() {
  let fixture = Fixture::new();
  let scope = RuleScope::similar(&fixture.command("cargo add serde --features derive")).expect("similar");
  let rules = [rule(1, scope)];
  for alike in [
    "cargo add tokio",
    "cargo add serde_json --dev",
    "git status && cargo add anyhow",
  ] {
    assert!(answering(&rules, &fixture.command(alike)).is_some(), "{alike}");
  }
  for other in [
    "cargo install ripgrep",
    "cargo add tokio && cargo test",
    "cargo add tokio > /etc/x",
    "cargo add $CRATE",
    "cargo add x && cat ~/.ssh/id_rsa",
  ] {
    assert!(answering(&rules, &fixture.command(other)).is_none(), "{other}");
  }
  assert_eq!(
    RuleScope::similar(&fixture.command("./scripts/regen.sh")),
    Err(SimilarUnavailable::Unknown)
  );
  assert_eq!(
    RuleScope::similar(&fixture.command("cargo add serde && cargo test")),
    Err(SimilarUnavailable::SeveralAsk)
  );
  assert_eq!(
    RuleScope::similar(&fixture.command("cargo add serde && git push")),
    Err(SimilarUnavailable::Destructive)
  );
  assert_eq!(
    RuleScope::similar(&fixture.command("cat ~/.ssh/id_rsa")),
    Err(SimilarUnavailable::Secrets)
  );
}

#[test]
fn approval_answers_similar_mcp_tool_is_server_and_tool() {
  let scope = RuleScope::similar(&mcp("pencil", "batch_design")).expect("similar");
  assert!(scope.revocable());
  let mut rules = vec![rule(7, scope)];
  assert_eq!(
    answering(&rules, &mcp("pencil", "batch_design")).map(|rule| rule.id),
    Some(ApprovalRuleId::new(7))
  );
  assert!(answering(&rules, &mcp("pencil", "export_nodes")).is_none());
  assert!(answering(&rules, &mcp("docs", "batch_design")).is_none());
  rules[0].revoked_at = Some(Timestamp::from_unix_millis(1));
  assert!(answering(&rules, &mcp("pencil", "batch_design")).is_none());
}

#[test]
fn approval_answers_always_allow_prefixes() {
  let fixture = Fixture::new();
  let ApprovalSubject::Command(add) = fixture.command("cargo add serde --features derive") else {
    panic!("a command")
  };
  let builder = listing(&["cargo test"]);
  let offered = always_prefixes(&add, builder.commands()).expect("offered");
  assert_eq!(offered[0].as_str(), "cargo add");
  let allowing = allowing_command(&builder, &add, &offered[0]).expect("allowed");
  let CommandPermission::Allowed(listed) = allowing.commands() else {
    panic!("a list")
  };
  assert_eq!(
    listed.entries().iter().map(CommandPrefix::as_str).collect::<Vec<_>>(),
    ["cargo test", "cargo add"]
  );
  let unrelated = CommandPrefix::new("rm").expect("prefix");
  assert_eq!(
    allowing_command(&builder, &add, &unrelated),
    Err(AlwaysAllowError::NotOffered)
  );
  let asks = AgentPermissions::new(RepositoryAccess::Write, CommandPermission::AskBefore, NetworkAccess::On);
  assert_eq!(
    always_prefixes(&add, asks.commands()),
    Err(AlwaysUnavailable::NoAllowedList)
  );
  for (command, unavailable) in [
    ("git push origin main", AlwaysUnavailable::Destructive),
    ("git reset src/lib.rs", AlwaysUnavailable::WiderForms),
    ("rm build/out.txt", AlwaysUnavailable::WiderForms),
    ("./regen.sh", AlwaysUnavailable::Unknown),
  ] {
    let ApprovalSubject::Command(classified) = fixture.command(command) else {
      panic!("a command")
    };
    assert_eq!(
      always_prefixes(&classified, builder.commands()),
      Err(unavailable),
      "{command}"
    );
  }
  // The run keeps allowing the prefix: its rule answers later commands it covers.
  let rules = [rule(
    2,
    RuleScope::AlwaysCommand {
      prefix: offered[0].clone(),
    },
  )];
  assert!(answering(&rules, &fixture.command("cargo add tokio")).is_some());
  assert!(!rules[0].scope.revocable());
}

#[test]
fn approval_answers_always_allow_mcp_tool() {
  let tools = ExemptTools::new(vec![McpToolName::new("get_screenshot").expect("name")]).expect("tools");
  let allowing = allowing_tool(&tools, "batch_design").expect("listed");
  assert!(allowing.contains("batch_design") && allowing.contains("get_screenshot"));
  assert_eq!(allowing_tool(&allowing, "batch_design").expect("again"), allowing);
  assert!(allowing_tool(&tools, "has space").is_err());
}

#[test]
fn approval_answers_auto_approval_never_approves_what_is_dangerous() {
  let fixture = Fixture::new();
  assert_eq!(auto_verdict(&mcp("pencil", "batch_design")), AutoVerdict::Approve(None));
  for (command, category) in [
    ("cargo test", CommandCategory::Worktree),
    ("git status", CommandCategory::ReadOnly),
  ] {
    assert_eq!(
      auto_verdict(&fixture.command(command)),
      AutoVerdict::Approve(Some(category)),
      "{command}"
    );
  }
  for (command, ask) in [
    ("cp src/a.rs /etc/a.rs", AutoAsk::OutsideWrite),
    ("echo x > ../outside.txt", AutoAsk::OutsideWrite),
    ("cat ~/.ssh/id_rsa", AutoAsk::Secrets),
    ("cat ~/.claude.json", AutoAsk::Secrets),
    ("ls ~/.codex", AutoAsk::Secrets),
    ("npm install left-pad", AutoAsk::Installs),
    ("cargo add serde", AutoAsk::Installs),
    ("pip install requests", AutoAsk::Installs),
    ("git push --force", AutoAsk::Destructive),
    ("git reset --hard", AutoAsk::Destructive),
    ("./regen.sh", AutoAsk::Unknown),
    ("echo $SECRET", AutoAsk::Unknown),
    ("git fetch origin", AutoAsk::Network),
    ("curl https://example.dev", AutoAsk::Network),
  ] {
    assert_eq!(
      auto_verdict(&fixture.command(command)),
      AutoVerdict::Ask(ask),
      "{command}"
    );
  }
}

#[test]
fn approval_answers_run_choice_wins_over_profile() {
  assert!(auto_approves(None, true));
  assert!(!auto_approves(None, false));
  assert!(auto_approves(Some(true), false));
  assert!(!auto_approves(Some(false), true));
  let mut profile = listing(&[]);
  assert!(!profile.auto_approve());
  profile.set_auto_approve(true);
  let stored = serde_json::to_string(&profile).expect("stored");
  let read: AgentPermissions = serde_json::from_str(&stored).expect("read");
  assert!(read.auto_approve());
  let narrowed = read.narrowed(RepositoryAccess::ReadOnly, &Default::default());
  assert!(narrowed.auto_approve());
  let before: AgentPermissions =
    serde_json::from_str(&serde_json::to_string(&listing(&[])).expect("stored")).expect("read without the setting");
  assert!(!before.auto_approve());
}

#[test]
fn approval_answers_deny_tells_the_agent_to_continue() {
  assert!(!ApprovalAnswer::Deny.approves());
  assert_eq!(ApprovalAnswer::Deny.decided_by(), DecidedBy::PersonDeny);
  assert!(!DecidedBy::PersonDeny.approves());
  assert_eq!(
    denial_message(&approval(false, "pencil", "batch_design")),
    "The person reviewing this run denied pencil batch_design. Continue without it."
  );
  assert_eq!(
    denial_message(&approval(true, "command", "cargo add serde")),
    "Denied by the person reviewing this run. Continue without it."
  );
  for decided in DecidedBy::ALL {
    assert_eq!(DecidedBy::from_str(decided.as_str()), Ok(decided));
  }
  let answer: ApprovalAnswer = serde_json::from_str(r#"{"kind":"always","prefix":"cargo add"}"#).expect("answer");
  assert_eq!(
    answer,
    ApprovalAnswer::Always {
      prefix: Some(CommandPrefix::new("cargo add").expect("prefix"))
    }
  );
}
