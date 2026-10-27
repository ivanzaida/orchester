//! The prompt's *Files and commands* part names what the step's CLI and permissions have: Claude Code's
//! file tools, and its edit tools only in a writing step; Codex's `apply_patch` only in a writing step; how to
//! merge the base branch safely and where scratch files go only in a writing step (ORC-54); nothing for a native
//! agent. Every step of either CLI is told how to run commands plainly, without a project instruction (ORC-95). A
//! resumed session already holds it.

use orchester_core::{
  connection::AgentProvider,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::InjectionEvent,
  run::{MERGE_SAFELY, PromptInput, RUN_PLAINLY, SCRATCH_FILES, WorkingTools},
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

fn claude_processes() -> &'static str {
  "\nFind apps you started with mcp__Orchester__list_run_processes and stop them with mcp__Orchester__stop_run_process(pid), instead of Stop-Process or kill. These tools run without asking and refuse executables outside this run's worktree, the runner and the agent CLI. Stop only apps you started. Drive that app with mcp__Orchester__call_run_app(pid, tool, arguments) instead of running tools/lurq-mcp-client.py. The relay finds discovery itself, keeps its token private and returns PNG paths under .tmp/."
}

fn codex_processes() -> &'static str {
  "\nFind apps you started with list_run_processes and stop them with stop_run_process(pid) of the Orchester MCP server, instead of Stop-Process or kill. These tools run without asking and refuse executables outside this run's worktree, the runner and the agent CLI. Stop only apps you started. Drive that app with call_run_app(pid, tool, arguments) instead of running tools/lurq-mcp-client.py. The relay finds discovery itself, keeps its token private and returns PNG paths under .tmp/."
}

/// A reading step's `part` without its process steering, `processes`, and how to run commands, on a line of its
/// own before it.
fn read<'a>(part: &'a str, processes: &str) -> &'a str {
  part
    .strip_suffix(processes)
    .expect("process steering")
    .strip_suffix(format!("\n{RUN_PLAINLY}").as_str())
    .expect("how to run commands")
}

/// A writing step's `part` without its process steering, `processes`, how to merge the base branch (ORC-54), where
/// scratch files go and how to run commands, each on a line of its own before the one after it.
fn written<'a>(part: &'a str, processes: &str) -> &'a str {
  let commands = part
    .strip_suffix(processes)
    .expect("process steering")
    .strip_suffix(format!("\n{MERGE_SAFELY}").as_str())
    .expect("how to merge the base")
    .strip_suffix(format!("\n{SCRATCH_FILES}").as_str())
    .expect("where scratch files go");
  read(commands, "")
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
    written(&claude_writes, claude_processes()),
    "Read, search and change files only with Orchester's file tools: mcp__Orchester__read_file, \
     mcp__Orchester__list_files, mcp__Orchester__search_file, mcp__Orchester__edit_file (after reading the file: give \
     the lines to replace and, as expected, exactly what they hold now), mcp__Orchester__create_file (new files \
     only), mcp__Orchester__delete_file and mcp__Orchester__move_file (one file each, never a folder). Claude Code's \
     own Read, Edit, Write, NotebookEdit, Grep and Glob are not available. Never read or change files through the \
     shell: no `cat`, `sed`, `awk`, heredocs, `rm` or `mv`. mcp__Orchester__read_file shows a PNG, JPEG or WebP file \
     as the image: open screenshots with it.\nCommands already run in the task's worktree: never \
     start one with `cd`. A plain command matches the allowed commands; other commands wait for a person's approval."
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
    read(&claude_reads, claude_processes()),
    "Read and search files only with Orchester's file tools: mcp__Orchester__read_file, \
     mcp__Orchester__list_files and mcp__Orchester__search_file. Claude Code's own Read, Grep and Glob are not \
     available; do not read files through the shell (`cat`, `sed`, `awk`). mcp__Orchester__read_file shows a PNG, \
     JPEG or WebP file as the image: open screenshots with it.\nCommands already run in the task's \
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
    written(&codex_writes, codex_processes()),
    "Read, search and change files with the read_file, list_files, search_file, edit_file (after read_file: give \
     the lines to replace and, as expected, exactly what they hold now), create_file (new files only), delete_file \
     and move_file (one file each, never a folder) tools of the Orchester MCP server, not through the shell (`cat`, \
     `rg`, `sed -i`, heredocs, `>`, `rm`, `mv`).\n\
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
    read(&codex_reads, codex_processes()),
    "Read and search files with the read_file, list_files and search_file tools of the Orchester MCP server, not \
     through the shell (`cat`, `rg`, `sed`, `awk`).\nCommands already run in the task's worktree: never start one with \
     `cd`."
  );
  // Every step of either CLI is told how to run commands, without a project instruction; a reading one keeps no
  // scratch files.
  for text in [&claude_writes, &claude_reads, &codex_writes, &codex_reads] {
    assert!(text.contains(RUN_PLAINLY), "{text}");
    assert!(RUN_PLAINLY.contains("one command per call") && RUN_PLAINLY.contains("`git stash`"));
  }
  assert!(!claude_reads.contains(SCRATCH_FILES) && !codex_reads.contains(SCRATCH_FILES));
  for provider in [AgentProvider::AnthropicApi, AgentProvider::OpenAiApi] {
    assert_eq!(tools(provider, RepositoryAccess::Write, listed()), None);
  }
  let resumed = tools(AgentProvider::ClaudeCodeCli, RepositoryAccess::Write, listed());
  assert_eq!(part(resumed, InjectionEvent::Resume), None);
  assert_eq!(part(None, start), None);
}
