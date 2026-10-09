//! The prompt's *Files and commands* part names what the step's CLI and permissions have: Claude Code's
//! file tools, and its edit tools only in a writing step; Codex's `apply_patch` only in a writing step;
//! nothing for a native agent. A resumed session already holds it.

use orchester_core::{
  connection::AgentProvider,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::InjectionEvent,
  run::{PromptInput, WorkingTools},
};

use crate::{
  fixture::task,
  prompt::{Prompt, builder, step_with},
};

fn permissions(repository: RepositoryAccess, commands: CommandPermission) -> AgentPermissions {
  AgentPermissions::new(repository, commands, NetworkAccess::Off)
}

fn listed() -> CommandPermission {
  let entry = CommandPrefix::new("cargo test").expect("prefix");
  CommandPermission::Allowed(AllowedCommands::new(vec![entry]).expect("list"))
}

/// The *Files and commands* part of a new session's prompt, if it has one.
fn part(working: Option<WorkingTools>, event: InjectionEvent) -> Option<String> {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let text = PromptInput {
    working,
    ..prompt.input(&[], &[], true)
  }
  .assemble(event)
  .expect("no variables")
  .text;
  let (_, after) = text.split_once("# Files and commands\n")?;
  let (part, _) = after
    .split_once("\n# When you finish\n")
    .expect("the result instructions follow");
  Some(part.trim().to_owned())
}

#[test]
fn working_prompt_names_the_tools_of_the_cli_and_the_access() {
  let start = InjectionEvent::Start;
  let tools = |provider, repository, commands| WorkingTools::new(provider, &permissions(repository, commands));
  let claude_writes = part(
    tools(AgentProvider::ClaudeCodeCli, RepositoryAccess::Write, listed()),
    start,
  )
  .expect("a part");
  assert_eq!(
    claude_writes,
    "Read, search and change files only with Orchester's file tools: mcp__Orchester__read_file, \
     mcp__Orchester__list_files, mcp__Orchester__search_file, mcp__Orchester__edit_file (after reading the file: give \
     the lines to replace and, as expected, exactly what they hold now) and mcp__Orchester__create_file (new files \
     only). Claude Code's own Read, Edit, Write, NotebookEdit, Grep and Glob are not available. Never read or change files through \
     the shell: no `cat`, `sed`, `awk` or heredocs.\nCommands already run in the task's worktree: never \
     start one with `cd`. Run one command per call, bare, without `;`, `&&` or variables, so that it matches the \
     allowed commands; other commands wait for a person's approval."
  );
  let claude_reads = part(
    tools(
      AgentProvider::ClaudeCodeCli,
      RepositoryAccess::ReadOnly,
      CommandPermission::AskBefore,
    ),
    start,
  )
  .expect("a part");
  assert_eq!(
    claude_reads,
    "Read and search files only with Orchester's file tools: mcp__Orchester__read_file, \
     mcp__Orchester__list_files and mcp__Orchester__search_file. Claude Code's own Read, Grep and Glob are not \
     available; do not read files through the shell (`cat`, `sed`, `awk`).\nCommands already run in the task's \
     worktree: never start one with `cd`. Each command waits for a \
     person's approval: run only those the step needs."
  );
  let codex_writes = part(
    tools(
      AgentProvider::CodexCli,
      RepositoryAccess::Write,
      CommandPermission::AnyInWorktree,
    ),
    start,
  )
  .expect("a part");
  assert_eq!(
    codex_writes,
    "Read, search and change files with the read_file, list_files, search_file, edit_file (after read_file: give \
     the lines to replace and, as expected, exactly what they hold now) and create_file (new files only) tools of \
     the Orchester MCP server, not through the shell (`cat`, `rg`, `sed -i`, heredocs, `>`).\n\
     Commands already run in the task's worktree: never start one with `cd`."
  );
  let codex_reads = part(
    tools(
      AgentProvider::CodexCli,
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
    ),
    start,
  )
  .expect("a part");
  assert_eq!(
    codex_reads,
    "Read and search files with the read_file, list_files and search_file tools of the Orchester MCP server, not \
     through the shell (`cat`, `rg`, `sed`, `awk`).\nCommands already run in the task's worktree: never start one with \
     `cd`."
  );
  for provider in [AgentProvider::AnthropicApi, AgentProvider::OpenAiApi] {
    assert_eq!(tools(provider, RepositoryAccess::Write, listed()), None);
  }
  let resumed = tools(AgentProvider::ClaudeCodeCli, RepositoryAccess::Write, listed());
  assert_eq!(part(resumed, InjectionEvent::Resume), None);
  assert_eq!(part(None, start), None);
}
