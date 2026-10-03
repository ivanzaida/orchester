//! A command the step's permissions leave to a person, headlessly: the real runner runs the step on
//! a fake CLI, which asks about one command, and "Needs you" shows the approval with what approving
//! means. In a step that only reads, it says that an approved command can still change files.

use orchester_core::{
  connection::AgentProvider,
  resource::{RepositoryAccess, Step, StepKind},
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, fake_claude, has, init_repository, programs, seed_agent},
};

/// A step that only reads, doing what `instructions` tell the fake CLI.
fn read_only(mut step: Step) -> Step {
  if let StepKind::Agent { repository, .. } = &mut step.kind {
    *repository = RepositoryAccess::ReadOnly;
  }
  step
}

/// The approval dialog of a run on `provider` whose one step does what `instructions` say.
fn approval_of(provider: AgentProvider, instructions: &'static str, reads: bool) -> Desktop {
  let (runner, fake) = programs();
  let fake = match provider {
    AgentProvider::CodexCli => fake,
    AgentProvider::ClaudeCodeCli => fake_claude(&fake),
    AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => panic!("the desktop tests have no native agent"),
  };
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_agent(repo, &fake, provider, |builder| {
      let step = agent_step(builder, "review", "Review", instructions, &[]);
      vec![if reads { read_only(step) } else { step }]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_for("tool-approval-dialog");
  desktop
}

const CLAUDE_READ_ONLY: &str =
  "This step only reads, but Claude Code runs an approved command outside any sandbox, where it can change files.";
const CODEX_READ_ONLY: &str =
  "This step only reads, but an approved command runs outside the sandbox, where it can change files.";

#[test]
fn command_approvals_claude_unlisted_command_in_a_read_only_step_says_it_can_change_files() {
  let mut desktop = approval_of(AgentProvider::ClaudeCodeCli, "Run `pnpm build`.", true);
  assert!(shows(&mut desktop, "tool-approval-dialog", "Approve command?"));
  assert!(shows(&mut desktop, "tool-approval-dialog", "pnpm build"));
  assert!(shows(&mut desktop, "tool-approval-dialog", CLAUDE_READ_ONLY));
}

#[test]
fn command_approvals_claude_unlisted_command_in_a_writing_step_asks_without_the_read_only_note() {
  let mut desktop = approval_of(AgentProvider::ClaudeCodeCli, "Run `pnpm build`.", false);
  assert!(shows(
    &mut desktop,
    "tool-approval-dialog",
    "policy asks you about commands that change the worktree"
  ));
  assert!(!shows(&mut desktop, "tool-approval-dialog", "This step only reads"));
}

#[test]
fn command_approvals_codex_escalation_in_a_read_only_step_says_it_can_change_files() {
  let mut desktop = approval_of(AgentProvider::CodexCli, "Escalate `pnpm install`.", true);
  assert!(shows(&mut desktop, "tool-approval-dialog", "pnpm install"));
  assert!(shows(
    &mut desktop,
    "tool-approval-dialog",
    "Do you want to reach the package registry?"
  ));
  assert!(shows(&mut desktop, "tool-approval-dialog", CODEX_READ_ONLY));
}

#[test]
fn command_approvals_approve_similar_from_the_conversation_card() {
  let mut desktop = approval_of(AgentProvider::ClaudeCodeCli, "Run `cargo add serde` twice.", false);
  let run = ProjectDb::open(desktop.repo())
    .expect("open")
    .pending_tool_approvals_all()
    .expect("requests")[0]
    .run;
  desktop.press("tool-approval-decide-later");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, "tool-approval-dialog"));
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-approval-similar");
  // The card offers the dialog's answers, Always allow… among them.
  assert!(has(&mut desktop, "conversation-approval-always"));
  desktop.click("conversation-approval-similar");
  desktop.wait_until("the similar rule answers the next request", |desktop| {
    let db = ProjectDb::open(desktop.repo()).expect("open");
    db.approval_rules(run).expect("rules").len() == 1 && db.pending_tool_approvals(run).expect("requests").is_empty()
  });
  assert!(!has(&mut desktop, "tool-approval-dialog"));
}
