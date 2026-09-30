//! The code index in a run: what new and resumed sessions are told about it, and the tokens an
//! agent used before its first file edit, followed from the events Codex and Claude Code report.

use orchester_core::{
  connection::stream::{AgentEvent, FileChange, FileChangeKind},
  resource::InjectionEvent,
  run::{CodeIndexPrompt, FirstEdit, FirstEditTracker, TokenUsage},
  time::Timestamp,
};

use crate::{
  fixture::task,
  prompt::{Prompt, builder, step_with},
};

const GIVEN: CodeIndexPrompt<'static> = CodeIndexPrompt::Given {
  revision: "4d2b8e0",
  folder: "/repo/.orchester/index/4d2b8e0",
  skill: "/repo/.orchester/skills/e1/_orchester/codebase-map/SKILL.md",
};

fn prompt() -> Prompt {
  Prompt::new(
    builder("Keep it small."),
    step_with("Implement it."),
    task("Users sign in with GitHub.", &["Sign-in works"]),
  )
}

#[test]
fn code_index_is_named_before_the_task_of_a_new_session() {
  let prompt = prompt();
  let mut input = prompt.input(&[], &[], true);
  input.code_index = Some(GIVEN);
  let text = input.assemble(InjectionEvent::Start).expect("assembled").text;

  let index = text.find("# Codebase index\n").expect("the index part");
  let task = text.find("# Task ORC-12").expect("the task part");
  assert!(index < task, "{text}");
  assert!(text.contains("at the run's base revision 4d2b8e0 is in /repo/.orchester/index/4d2b8e0:"));
  assert!(text.contains("The codebase-map skill (/repo/.orchester/skills/e1/_orchester/codebase-map/SKILL.md)"));
}

#[test]
fn code_index_is_named_again_when_a_session_resumes() {
  let prompt = prompt();
  let mut input = prompt.input(&[], &[], true);
  input.code_index = Some(GIVEN);
  let text = input.assemble(InjectionEvent::Resume).expect("assembled").text;

  assert!(text.contains("# Codebase index\n"), "{text}");
  assert!(text.contains("/repo/.orchester/index/4d2b8e0"));
}

#[test]
fn without_ripgrep_the_prompt_says_to_search_with_git_grep() {
  let prompt = prompt();
  let mut input = prompt.input(&[], &[], true);
  input.code_index = Some(CodeIndexPrompt::WithoutRipgrep);
  let text = input.assemble(InjectionEvent::Start).expect("assembled").text;

  assert!(text.contains("# Searching the repository\n"), "{text}");
  assert!(text.contains("`git grep -n`"));
  assert!(!text.contains("# Codebase index"));
}

#[test]
fn a_prompt_without_the_code_index_says_nothing_of_it() {
  let prompt = prompt();
  let text = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Start)
    .expect("assembled")
    .text;
  assert!(!text.contains("# Codebase index"));
  assert!(!text.contains("# Searching the repository"));
}

fn usage(input: u64, cached_input: u64, output: u64) -> TokenUsage {
  TokenUsage {
    input,
    cached_input,
    output,
  }
}

fn completed(input: u64) -> AgentEvent {
  AgentEvent::TurnCompleted {
    usage: usage(input, 0, 1),
  }
}

fn edited(path: &str) -> AgentEvent {
  AgentEvent::FileChanged {
    item: "item".into(),
    changes: vec![FileChange {
      path: path.into(),
      kind: FileChangeKind::Modified,
    }],
  }
}

fn follow(events: &[AgentEvent]) -> FirstEditTracker {
  let mut tracker = FirstEditTracker::default();
  for (at, event) in events.iter().enumerate() {
    tracker.observe(event, Timestamp::from_unix_millis(i64::try_from(at).expect("small")));
  }
  tracker
}

#[test]
fn first_edit_counts_a_codex_turn_that_edits_before_it_reports_its_usage() {
  // `codex exec` reports a turn's usage when the turn completes, after the edits in it.
  let tracker = follow(&[
    AgentEvent::TurnStarted,
    completed(100),
    AgentEvent::TurnStarted,
    edited("src/a.rs"),
    edited("src/b.rs"),
    completed(40),
    AgentEvent::TurnStarted,
    completed(7),
  ]);
  let progress = tracker.progress();
  assert_eq!(progress.before, usage(140, 0, 2));
  assert_eq!(
    progress.first_edit,
    Some(FirstEdit {
      at: Timestamp::from_unix_millis(3),
      path: "src/a.rs".into(),
    })
  );
}

#[test]
fn first_edit_counts_the_claude_messages_up_to_the_edit() {
  // Claude Code reports each message with its usage, and an edit once its result arrives.
  let tracker = follow(&[
    AgentEvent::TurnStarted,
    completed(100),
    AgentEvent::TurnStarted,
    completed(30),
    edited("/repo/.orchester/worktrees/run/src/a.rs"),
    AgentEvent::TurnStarted,
    completed(50),
    completed(5),
  ]);
  assert_eq!(tracker.progress().before, usage(130, 0, 2));
}

#[test]
fn first_edit_of_an_agent_that_changed_no_file_counts_everything() {
  let tracker = follow(&[
    AgentEvent::TurnStarted,
    completed(100),
    AgentEvent::TurnStarted,
    completed(20),
  ]);
  assert_eq!(tracker.progress().before, usage(120, 0, 2));
  assert_eq!(tracker.progress().first_edit, None);
}
