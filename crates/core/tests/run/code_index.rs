//! The code index in a run: what new and resumed sessions are told about it, and the tokens an
//! agent used before its first file edit, followed from the events Codex and Claude Code report.

use orchester_core::{
  code_index::RipgrepFallback,
  connection::{
    AgentProvider,
    stream::{AgentEvent, FileChange, FileChangeKind, RunnerProblem},
  },
  resource::InjectionEvent,
  run::{CodeIndexPrompt, EditProgress, FirstEdit, FirstEditTracker, TokenUsage},
  time::Timestamp,
};

use crate::{
  fixture::task,
  prompt::{Prompt, builder, step_with},
};

const GIVEN: CodeIndexPrompt<'static> = given("", false);

/// The index of a fixture repository with `map` as its `map.md`.
const fn given(map: &str, git_grep: bool) -> CodeIndexPrompt<'_> {
  CodeIndexPrompt::Given {
    revision: "4d2b8e0",
    folder: "/repo/.orchester/index/4d2b8e0",
    skill: "/repo/.orchester/skills/e1/_orchester/codebase-index/SKILL.md",
    git_grep,
    map,
  }
}

const MAP: &str = "# Repository map\n\n\
                   Revision 4d2b8e0: 15 files, 168 lines, 42 symbols, 2 packages.\n\n\
                   ## crates/app: app (rust)\n\n\
                   The fixture workspace.\n\n\
                   - files: 4, lines: 62, symbols: 18\n\
                   - entry: src/lib.rs (lib), src/main.rs (bin app)\n\
                   - tests: tests/ (1 file)\n\
                   - modules in crates/app/src/:\n\
                   \x20 - lib.rs (56 lines)\n\
                   \x20 - store/ (2 files, 90 lines): mod.rs, sqlite.rs\n\
                   - also: Cargo.toml\n\n\
                   ## Outside packages\n\n\
                   - docs/ (1 file)\n\
                   - root files: .gitignore, Cargo.toml\n";

/// The prompt's bound on the map excerpt, in bytes.
const MAP_EXCERPT_LIMIT: usize = 8 * 1024;

fn prompt() -> Prompt {
  Prompt::new(
    builder("Keep it small."),
    step_with("Implement it."),
    task("Users sign in with GitHub.", &["Sign-in works"]),
  )
}

/// The text of a new session's prompt told about an index whose `map.md` is `map`.
fn with_map(map: &str) -> String {
  let prompt = prompt();
  let mut input = prompt.input(&[], &[], true);
  input.code_index = Some(given(map, false));
  input.assemble(InjectionEvent::Start).expect("assembled").text
}

/// The line that introduces the map excerpt in a prompt.
const EXCERPT_INTRO: &str = "The packages and modules, from map.md";
/// What the introduction adds when the module folders lost what they hold.
const CONTENTS_OMITTED: &str = " (module file lists omitted; see map.md)";

/// The map excerpt of the prompt `text`.
fn excerpt(text: &str) -> &str {
  let intro = text
    .find(EXCERPT_INTRO)
    .unwrap_or_else(|| panic!("no map excerpt in\n{text}"));
  let start = intro + text[intro..].find(":\n```text\n").expect("the excerpt is fenced") + ":\n```text\n".len();
  let end = start + text[start..].find("```\n").expect("the excerpt ends");
  &text[start..end]
}

/// The pointer a prompt without an excerpt has to `map.md`.
const MAP_POINTER: &str = "Read map.md in /repo/.orchester/index/4d2b8e0 first";

/// A map of `packages` packages of `modules` module folders each.
fn large_map(packages: usize, modules: usize) -> String {
  let mut map = String::from("# Repository map\n\nRevision 4d2b8e0: many files.\n");
  for package in 0..packages {
    map.push_str(&format!(
      "\n## crates/part{package}: part{package} (rust)\n\n- files: 9\n- modules in crates/part{package}/src/:\n"
    ));
    for module in 0..modules {
      map.push_str(&format!(
        "  - module{module}/ (3 files, 300 lines): first.rs, second.rs, third.rs\n"
      ));
    }
  }
  map
}

#[test]
fn code_index_prompt_carries_the_package_and_module_map() {
  let text = with_map(MAP);
  assert_eq!(
    excerpt(&text),
    "## crates/app: app (rust)\n\
     The fixture workspace.\n\
     - entry: src/lib.rs (lib), src/main.rs (bin app)\n\
     - tests: tests/ (1 file)\n\
     - modules in crates/app/src/:\n\
     \x20 - lib.rs (56 lines)\n\
     \x20 - store/ (2 files, 90 lines): mod.rs, sqlite.rs\n\
     ## Outside packages\n\
     - docs/ (1 file)\n\
     - root files: .gitignore, Cargo.toml\n"
  );
  assert!(
    text.contains("symbols-consts.tsv") && text.contains("symbols-tests.tsv"),
    "{text}"
  );
  assert!(text.contains(&format!("{EXCERPT_INTRO}:\n")), "{text}");
  assert!(!text.contains(MAP_POINTER), "{text}");
}

#[test]
fn a_map_over_the_bound_loses_what_module_folders_hold_first() {
  let map = large_map(5, 30);
  assert!(map.len() > MAP_EXCERPT_LIMIT, "the test map is over the bound");
  let text = with_map(&map);
  let excerpt = excerpt(&text);
  assert!(excerpt.len() <= MAP_EXCERPT_LIMIT, "{} bytes", excerpt.len());
  assert!(excerpt.contains("## crates/part4: part4 (rust)\n"));
  assert!(excerpt.contains("  - module29/ (3 files, 300 lines)\n"));
  assert!(!excerpt.contains("first.rs"));
  assert!(!excerpt.contains("(the rest is in map.md)"));
  assert!(
    text.contains(&format!("{EXCERPT_INTRO}{CONTENTS_OMITTED}:\n")),
    "{text}"
  );
}

#[test]
fn a_map_too_large_even_without_module_contents_is_cut_at_a_line() {
  let text = with_map(&large_map(20, 40));
  let excerpt = excerpt(&text);
  assert!(excerpt.len() <= MAP_EXCERPT_LIMIT, "{} bytes", excerpt.len());
  assert!(excerpt.starts_with("## crates/part0: part0 (rust)\n"));
  assert!(excerpt.ends_with("\n- ... (the rest is in map.md)\n"), "{excerpt}");
  assert!(!excerpt.contains("## crates/part19"));
  assert!(
    text.contains(&format!("{EXCERPT_INTRO}{CONTENTS_OMITTED}:\n")),
    "{text}"
  );
}

#[test]
fn code_index_without_a_map_excerpt_points_to_map_md() {
  let unread = with_map("");
  let without_sections = with_map("# Repository map\n\nRevision 4d2b8e0: 0 files, 0 lines, 0 symbols, 0 packages.\n");
  for text in [unread, without_sections] {
    assert!(text.contains("# Codebase index\n"));
    assert!(!text.contains(EXCERPT_INTRO), "{text}");
    assert_eq!(text.matches(MAP_POINTER).count(), 1, "{text}");
  }
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
  assert!(
    text.contains("The orchester-codebase-index skill (/repo/.orchester/skills/e1/_orchester/codebase-index/SKILL.md)")
  );
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
fn code_index_without_ripgrep_names_git_grep_for_the_index_and_the_repository() {
  let prompt = prompt();
  let mut input = prompt.input(&[], &[], true);
  input.code_index = Some(given("", true));
  let text = input.assemble(InjectionEvent::Start).expect("assembled").text;
  assert!(text.contains("# Codebase index\n"), "{text}");
  assert!(text.contains("`git grep --no-index -n`"));
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
  assert!(progress.at_most, "the turn went on after the edit");
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
  assert!(!tracker.progress().at_most);
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

#[test]
fn first_edit_of_an_execution_adds_up_its_invocations_until_the_edit() {
  let edit = FirstEdit {
    at: Timestamp::from_unix_millis(5),
    path: "src/a.rs".into(),
  };
  let parts = [
    EditProgress {
      before: usage(100, 10, 1),
      ..EditProgress::default()
    },
    EditProgress {
      before: usage(40, 0, 2),
      first_edit: Some(edit.clone()),
      at_most: true,
    },
    EditProgress {
      before: usage(7, 0, 3),
      ..EditProgress::default()
    },
  ];
  let execution = EditProgress::of_execution(&parts);
  assert_eq!(execution.before, usage(140, 10, 3));
  assert_eq!(execution.first_edit, Some(edit));
  assert!(execution.at_most);
  assert_eq!(EditProgress::of_execution(&parts[..1]).first_edit, None);
}

#[test]
fn a_missing_ripgrep_names_how_the_agent_searches_instead() {
  assert_eq!(
    RipgrepFallback::of(AgentProvider::CodexCli, true),
    RipgrepFallback::IndexWithGitGrep
  );
  assert_eq!(
    RipgrepFallback::of(AgentProvider::CodexCli, false),
    RipgrepFallback::GitGrep
  );
  assert_eq!(
    RipgrepFallback::of(AgentProvider::ClaudeCodeCli, true),
    RipgrepFallback::IndexWithOwnTools
  );
  assert_eq!(
    RipgrepFallback::of(AgentProvider::ClaudeCodeCli, false),
    RipgrepFallback::OwnTools
  );
  // A problem recorded before the fallback was kept still reads, not knowing it.
  let stored: RunnerProblem = serde_json::from_str(r#"{"problem":"ripgrep_not_found"}"#).expect("reads");
  assert_eq!(
    stored,
    RunnerProblem::RipgrepNotFound {
      fallback: RipgrepFallback::Unknown
    }
  );
  let stored: RunnerProblem =
    serde_json::from_str(r#"{"problem":"ripgrep_unavailable","detail":"denied"}"#).expect("reads");
  assert_eq!(
    stored,
    RunnerProblem::RipgrepUnavailable {
      detail: "denied".into(),
      fallback: RipgrepFallback::Unknown
    }
  );
}
