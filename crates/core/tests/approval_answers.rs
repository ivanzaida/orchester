use std::str::FromStr;

use orchester_core::{
  connection::mcp::{ExemptTools, McpToolName},
  id::{InvocationId, RunId, StepExecutionId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{CommandCategory, CommandScope},
  },
  run::{
    AlwaysAllowError, AlwaysUnavailable, Answerer, ApprovalAnswer, ApprovalRule, ApprovalRuleId, ApprovalSubject,
    AutoAsk, AutoVerdict, AutomationAnswerError, DecidedBy, DenialNote, DenialNoteError, PendingToolApproval,
    RuleScope, SimilarUnavailable, ToolReach, allowing_command, allowing_tool, always_prefixes, answering,
    auto_approves, auto_verdict, denial_message, denial_with_message,
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
    reach: ToolReach::Any,
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

/// Approve similar of one of Orchester's calls outside the worktree covers its reads and searches in the folder the
/// call read in and below it, nowhere else. The folder comes from the request's target, and only for those tools.
#[test]
fn approval_answers_similar_external_read_keeps_to_its_folder() {
  let read_in = |folder: &str| {
    let mut request = approval(false, "Orchester", "read_external_file");
    request.target = Some(folder.to_owned());
    ApprovalSubject::of(&request, &Fixture::new().scope)
  };
  let deps = std::path::Path::new("deps").join("serde");
  let text = |path: &std::path::Path| path.to_string_lossy().into_owned();
  let first = read_in(&text(&deps));
  assert_eq!(
    first,
    ApprovalSubject::McpTool {
      server: "Orchester".to_owned(),
      tool: "read_external_file".to_owned(),
      reach: ToolReach::Folder {
        folder: text(&deps),
        broad: false
      },
    }
  );
  let rules = vec![rule(3, RuleScope::similar(&first).expect("similar"))];
  assert!(answering(&rules, &read_in(&text(&deps))).is_some());
  assert!(answering(&rules, &read_in(&text(&deps.join("src")))).is_some());
  assert!(
    answering(
      &rules,
      &read_in(&text(&std::path::Path::new("deps").join("serde_json")))
    )
    .is_none()
  );
  assert!(answering(&rules, &read_in("elsewhere")).is_none());
  let mut search = approval(false, "Orchester", "search_external_file");
  search.target = Some(text(&deps));
  assert!(answering(&rules, &ApprovalSubject::of(&search, &Fixture::new().scope)).is_some());
  assert!(answering(&rules, &mcp("Orchester", "read_external_file")).is_none());
  // Another server's target is the file it names, never a folder a rule keeps to.
  let mut pencil = approval(false, "pencil", "batch_design");
  pencil.target = Some("design.pen".to_owned());
  assert_eq!(
    ApprovalSubject::of(&pencil, &Fixture::new().scope),
    mcp("pencil", "batch_design")
  );
  // A rule made before folders were kept covers every call of its tool.
  let stored: RuleScope =
    serde_json::from_str(r#"{"kind":"similar_mcp_tool","server":"Orchester","tool":"read_external_file"}"#)
      .expect("an earlier rule reads");
  assert!(answering(&[rule(4, stored)], &read_in("anywhere")).is_some());
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
    ("cp src/a.rs ../../a.rs", AutoAsk::OutsideWrite),
    // Git Bash places a path rooted without a drive under its own installation: not placed.
    (
      "cp src/a.rs /etc/a.rs",
      if cfg!(windows) {
        AutoAsk::Unknown
      } else {
        AutoAsk::OutsideWrite
      },
    ),
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
    denial_message(&approval(false, "pencil", "batch_design"), Answerer::Person),
    "The person reviewing this run denied pencil batch_design. Continue without it."
  );
  assert_eq!(
    denial_message(&approval(true, "command", "cargo add serde"), Answerer::Person),
    "Denied by the person reviewing this run. Continue without it."
  );
  // Automation's denial invents no person.
  assert_eq!(
    denial_message(&approval(false, "Orchester", "git_commit"), Answerer::Automation),
    "Automation denied Orchester git_commit through Orchester's MCP server. Continue without it."
  );
  assert_eq!(
    denial_message(&approval(true, "command", "cargo add serde"), Answerer::Automation),
    "Denied by automation through Orchester's MCP server. Continue without it."
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

/// A denial may carry the denier's words (ORC-29): trimmed, not empty, at most 2000 characters; the agent is told who
/// denied it, then the words. Who may deny may deny with a message, automation too.
#[test]
fn approval_answers_deny_with_a_message_tells_the_words() {
  let note = DenialNote::new("  Use serde_json instead.\n").expect("words");
  assert_eq!(note.as_str(), "Use serde_json instead.");
  assert_eq!(DenialNote::new(" \n "), Err(DenialNoteError::Empty));
  let long = "x".repeat(DenialNote::MAX_CHARS + 1);
  assert_eq!(
    DenialNote::new(&long),
    Err(DenialNoteError::TooLong {
      chars: DenialNote::MAX_CHARS + 1
    })
  );
  assert!(DenialNote::new(&"ä".repeat(DenialNote::MAX_CHARS)).is_ok());
  let answer = ApprovalAnswer::DenyWithMessage { message: note.clone() };
  assert!(!answer.approves());
  assert_eq!(answer.decided_by(), DecidedBy::PersonDeny);
  assert_eq!(answer.denial_note(), Some(&note));
  assert_eq!(ApprovalAnswer::Deny.denial_note(), None);
  let fixture = Fixture::new();
  assert_eq!(
    answer.given_by(Answerer::Automation, &fixture.command("git push --force")),
    Ok(DecidedBy::AutomationDeny)
  );
  assert_eq!(
    denial_with_message(&approval(true, "command", "cargo add serde"), Answerer::Person, &note),
    "Denied by the person reviewing this run: Use serde_json instead."
  );
  assert_eq!(
    denial_with_message(&approval(false, "pencil", "batch_design"), Answerer::Person, &note),
    "The person reviewing this run denied pencil batch_design: Use serde_json instead."
  );
  assert_eq!(
    denial_with_message(
      &approval(true, "command", "cargo add serde"),
      Answerer::Automation,
      &note
    ),
    "Denied by automation through Orchester's MCP server: Use serde_json instead."
  );
  let wire = serde_json::to_string(&answer).expect("wire");
  assert_eq!(
    wire,
    r#"{"kind":"deny_with_message","message":"Use serde_json instead."}"#
  );
  assert_eq!(serde_json::from_str::<ApprovalAnswer>(&wire).expect("read"), answer);
  assert!(serde_json::from_str::<ApprovalAnswer>(r#"{"kind":"deny_with_message","message":"  "}"#).is_err());
}

/// A person, automation and auto mode's reviewer answer a request that waited for them; the step's
/// permissions, a rule of the run and auto-approval answer it as it comes, so the Conversation shows it
/// in the agent's tool chip, not as a card (ORC-71).
#[test]
fn approval_answers_who_answered_a_waiting_request() {
  let waited: Vec<DecidedBy> = DecidedBy::ALL
    .into_iter()
    .filter(|decided| decided.answered_waiting())
    .collect();
  assert_eq!(
    waited,
    [
      DecidedBy::Person,
      DecidedBy::PersonSimilar,
      DecidedBy::PersonAlways,
      DecidedBy::PersonRun,
      DecidedBy::PersonDeny,
      DecidedBy::Automation,
      DecidedBy::AutomationDeny,
      DecidedBy::Reviewer,
    ]
  );
}

/// Automation approves once only what auto-approval would approve; it denies anything; the rest, and the
/// wider answers, stay with a person, who answers every request.
#[test]
fn approval_answers_automation_approves_only_what_auto_approval_would() {
  let fixture = Fixture::new();
  for subject in [
    mcp("pencil", "batch_design"),
    fixture.command("cargo test"),
    fixture.command("git status"),
  ] {
    assert_eq!(
      ApprovalAnswer::Once.given_by(Answerer::Automation, &subject),
      Ok(DecidedBy::Automation)
    );
  }
  for (command, ask) in [
    ("cat ~/.ssh/id_rsa", AutoAsk::Secrets),
    ("git push --force", AutoAsk::Destructive),
    ("./regen.sh", AutoAsk::Unknown),
    ("echo x > ../outside.txt", AutoAsk::OutsideWrite),
    ("npm install left-pad", AutoAsk::Installs),
    ("curl https://example.dev", AutoAsk::Network),
  ] {
    let subject = fixture.command(command);
    assert_eq!(
      ApprovalAnswer::Once.given_by(Answerer::Automation, &subject),
      Err(AutomationAnswerError::PersonOnly(ask)),
      "{command}"
    );
    assert_eq!(
      ApprovalAnswer::Deny.given_by(Answerer::Automation, &subject),
      Ok(DecidedBy::AutomationDeny),
      "{command}"
    );
    assert_eq!(
      ApprovalAnswer::Once.given_by(Answerer::Person, &subject),
      Ok(DecidedBy::Person),
      "{command}"
    );
  }
  let subject = fixture.command("cargo test");
  for wider in [
    ApprovalAnswer::Similar,
    ApprovalAnswer::ApproveRun,
    ApprovalAnswer::Always { prefix: None },
  ] {
    assert_eq!(
      wider.given_by(Answerer::Automation, &subject),
      Err(AutomationAnswerError::Wider)
    );
  }
}
