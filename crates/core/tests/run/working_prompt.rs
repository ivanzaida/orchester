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
    "Read, search and edit files with your own tools (Read, Grep, Glob, Edit, Write), not through the shell: no \
     `cat`, `sed`, `awk` or heredocs to read or change files.\nCommands already run in the task's worktree: never \
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
    "Read and search files with your own tools (Read, Grep, Glob), not through the shell (`cat`, `sed`, \
     `awk`).\nCommands already run in the task's worktree: never start one with `cd`. Each command waits for a \
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
    "Change files with apply_patch, not through the shell (`sed -i`, heredocs, `>`).\nCommands already run in the \
     task's worktree: never start one with `cd`."
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
    "Commands already run in the task's worktree: never start one with `cd`."
  );
  for provider in [AgentProvider::AnthropicApi, AgentProvider::OpenAiApi] {
    assert_eq!(tools(provider, RepositoryAccess::Write, listed()), None);
  }
  let resumed = tools(AgentProvider::ClaudeCodeCli, RepositoryAccess::Write, listed());
  assert_eq!(part(resumed, InjectionEvent::Resume), None);
  assert_eq!(part(None, start), None);
}
