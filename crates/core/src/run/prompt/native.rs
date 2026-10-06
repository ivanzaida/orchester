//! A native agent's prompt (`docs/architecture/native-agent.md` §6.1): the same composition as an agent
//! CLI's, with the result handed in through `finish_step`, split in two. The system prompt, frozen for
//! the conversation, is a short preamble on how Orchester runs the agent, then the role, project rules,
//! the repository's `AGENTS.md` while the project lets agents read it, pipeline and step; the first user
//! item is the rest: the code index, skills, task memory and the task. Neither part carries a time or an
//! id that changes between requests, so the provider's cache holds.

use super::{AssembledPrompt, Composition, PromptAssemblyError, PromptInput, ResultDelivery};
use crate::{
  connection::invocation::Platform,
  resource::InjectionEvent,
  run::{CommandShell, InvocationPrompt},
};

/// What a native agent is told about where it runs, besides the step's prompt.
#[derive(Clone, Copy, Debug)]
pub struct NativeSetting<'a> {
  pub platform: Platform,
  /// The shell `shell: true` commands go to; `None` when this computer has none of them.
  pub shell: Option<CommandShell>,
  /// The repository's `AGENTS.md`, while Repository agent files is on and the worktree has one.
  pub repository_instructions: Option<&'a str>,
}

/// A native conversation's first prompt: its system prompt and its first user item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePrompt {
  pub system: String,
  pub task: String,
  pub sources: InvocationPrompt,
}

impl PromptInput<'_> {
  /// The prompt a new native conversation starts with.
  pub fn assemble_native(&self, setting: NativeSetting<'_>) -> Result<NativePrompt, PromptAssemblyError> {
    let composition = Composition::new(self, InjectionEvent::Start, ResultDelivery::Tool);
    let (AssembledPrompt { mut text, sources }, task_at) = composition.split()?;
    let task = text.split_off(task_at).trim().to_owned();
    let mut system = preamble(setting);
    if let Some(instructions) = setting
      .repository_instructions
      .map(str::trim)
      .filter(|text| !text.is_empty())
    {
      system.push_str("\n\n# Repository instructions: AGENTS.md\n\n");
      system.push_str(instructions);
    }
    let rest = text.trim();
    if !rest.is_empty() {
      system.push_str("\n\n");
      system.push_str(rest);
    }
    Ok(NativePrompt { system, task, sources })
  }
}

/// How Orchester runs the agent: its tools, the computer and shell, and how the step ends.
fn preamble(setting: NativeSetting<'_>) -> String {
  let computer = match setting.platform {
    Platform::Windows => "Windows",
    Platform::MacOs => "macOS",
  };
  let shell = match setting.shell {
    Some(shell) => format!(
      "set shell to true: the string then runs in {} ({}).",
      shell.name(),
      shell.invocation()
    ),
    None => "no shell is available on this computer, so run each program directly.".to_owned(),
  };
  format!(
    "You are a coding agent working on one step of a task, in a git worktree on a {computer} computer. Orchester \
     runs the tools you call and keeps them within this step's permissions.\n\n\
     - Paths are relative to the worktree root. read_file, list_files and search read the worktree, the code \
     index and your skills; edit_file and write_file change files in the worktree, when the step may.\n\
     - Read a file with read_file before you edit or replace it. An edit fails when the file changed since \
     you read it: read it again.\n\
     - run_command starts one program without a shell. For pipes, redirection or several commands, {shell} A \
     command outside the allowed list waits for a person's approval; when it is denied, continue without it.\n\
     - End the step by calling finish_step. When you need the person's answer to go on, call ask_question."
  )
}
