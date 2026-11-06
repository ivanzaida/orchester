//! A native agent's `run_command` (`docs/architecture/native-agent.md` §4.6): the program found on the
//! agent's `PATH` (ripgrep's folder first) and started without a shell, or the string run by the platform
//! shell (`pwsh`, else Windows PowerShell 5.1; zsh on macOS, decision 3). Each command starts under the
//! supervisor, so the Windows job object and the macOS process group end what it started; its standard
//! input is empty, its environment the cleaned one every agent gets plus the project's variables, never a
//! secret. It is ended at its time limit. The model gets the head and tail of its output, 32 KiB in all,
//! with what was left out; Activity gets up to 1 MiB.

use std::{
  collections::VecDeque,
  ffi::OsString,
  path::{Path, PathBuf},
  time::{Duration, Instant},
};

use orchester_core::{
  command::Command,
  connection::invocation::Platform,
  native::{
    command::{CommandForm, ParsedCommand},
    session::ToolOutcome,
  },
  resource::CommandProgram,
  run::CommandShell,
};

use crate::supervisor::{ProcessEvent, ProcessExit, Supervised};

/// Bytes of output the model gets from the start, and from the end, of a command's output.
const MODEL_HALF: usize = 16 * 1024;
/// Bytes of output Activity keeps.
const ACTIVITY_LIMIT: usize = 1024 * 1024;

/// Where a native invocation's commands run.
pub(in super::super) struct CommandHost {
  environment: Vec<(String, String)>,
  /// The `PATH` programs are looked up on and start with.
  path: Option<OsString>,
  worktree: PathBuf,
  shell: Option<(CommandShell, PathBuf)>,
}

impl CommandHost {
  /// Commands of an agent in `worktree` with `environment`, their shell found on its `PATH`.
  pub(in super::super) fn new(environment: Vec<(String, String)>, worktree: &Path) -> Self {
    let path = environment
      .iter()
      .rev()
      .find(|(name, _)| name.eq_ignore_ascii_case("PATH"))
      .map(|(_, value)| OsString::from(value));
    let shell = platform_shell(path.as_deref(), worktree);
    Self {
      environment,
      path,
      worktree: worktree.to_path_buf(),
      shell,
    }
  }

  /// The shell `shell: true` commands go to; `None` when none was found.
  pub(in super::super) fn shell(&self) -> Option<CommandShell> {
    self.shell.as_ref().map(|(shell, _)| *shell)
  }

  /// A call that failed without running anything, with `message` as its result.
  pub(in super::super) fn failed(message: String) -> ToolOutcome {
    ToolOutcome {
      content: message.clone(),
      is_error: true,
      activity: message,
      exit_code: None,
      changes: Vec::new(),
      detail: None,
    }
  }

  /// Runs `command` to its end or its time limit.
  pub(in super::super) async fn run(&self, command: &ParsedCommand) -> ToolOutcome {
    let started = Instant::now();
    let process = match self.process(command) {
      Ok(process) => process,
      Err(message) => return Self::failed(message),
    };
    let mut supervised = match Supervised::start(&process) {
      Ok(supervised) => supervised,
      Err(error) => return Self::failed(error.to_string()),
    };
    let limit = Duration::from_secs(u64::from(command.timeout_seconds()));
    let mut output = Output::default();
    let (exit, timed_out) = match tokio::time::timeout(limit, collect(&mut supervised, &mut output)).await {
      Ok(exit) => (exit, false),
      Err(_) => {
        supervised.terminate();
        (collect(&mut supervised, &mut output).await, true)
      }
    };
    output.outcome(&exit, timed_out.then_some(limit), started.elapsed())
  }

  /// The process `command` starts: its program found on the `PATH` with its arguments, or its string
  /// handed to the shell.
  fn process(&self, command: &ParsedCommand) -> Result<Command, String> {
    let (program, args) = match (command.form(), command.program_args()) {
      (CommandForm::Shell, _) => {
        let (shell, program) = self
          .shell
          .as_ref()
          .ok_or_else(|| "no shell was found on this computer; run the program directly".to_owned())?;
        let mut args: Vec<String> = shell_options(*shell)
          .iter()
          .map(|option| (*option).to_owned())
          .collect();
        args.push(command.text().to_owned());
        (program.clone(), args)
      }
      (CommandForm::Direct { .. }, Some(argv)) => {
        let (name, args) = argv
          .split_first()
          .ok_or_else(|| "the command names no program".to_owned())?;
        (self.find(name)?, args.to_vec())
      }
      (CommandForm::Direct { .. }, None) => return Err("the command names no program".to_owned()),
    };
    Ok(
      Command::new(&program, args)
        .with_current_dir(command.workdir())
        .with_environment(self.environment.clone()),
    )
  }

  /// The program file `name` is, on the `PATH` or in the worktree.
  fn find(&self, name: &str) -> Result<PathBuf, String> {
    let program = CommandProgram::new(name).map_err(|error| error.to_string())?;
    program
      .candidates(&self.worktree, self.path.as_deref(), Platform::current())
      .into_iter()
      .find(|candidate| candidate.is_file())
      .ok_or_else(|| format!("{name} was not found on the PATH or in the worktree"))
  }
}

/// The shell of this platform (decision 3): `pwsh` when it is on the `PATH`, else Windows PowerShell
/// 5.1, on Windows; zsh on macOS.
fn platform_shell(path: Option<&std::ffi::OsStr>, worktree: &Path) -> Option<(CommandShell, PathBuf)> {
  let found = |name: &str| {
    CommandProgram::new(name)
      .ok()?
      .candidates(worktree, path, Platform::current())
      .into_iter()
      .find(|candidate| candidate.is_file())
  };
  match Platform::current() {
    Platform::Windows => found("pwsh")
      .map(|program| (CommandShell::Pwsh, program))
      .or_else(|| found("powershell").map(|program| (CommandShell::WindowsPowerShell, program))),
    Platform::MacOs => {
      let zsh = PathBuf::from("/bin/zsh");
      zsh.is_file().then_some((CommandShell::Zsh, zsh))
    }
  }
}

/// What a shell's program is given before the command string.
const fn shell_options(shell: CommandShell) -> &'static [&'static str] {
  match shell {
    CommandShell::Pwsh | CommandShell::WindowsPowerShell => &["-NoProfile", "-NonInteractive", "-Command"],
    CommandShell::Zsh => &["-c"],
  }
}

/// Reads `supervised`'s output into `output` until it exits.
async fn collect(supervised: &mut Supervised, output: &mut Output) -> ProcessExit {
  loop {
    match supervised.next().await {
      Some(ProcessEvent::Output { line, .. }) => output.push(&line),
      Some(ProcessEvent::Exited(exit)) => return exit,
      None => {
        return ProcessExit::WaitFailed {
          reason: "the process's events ended".to_owned(),
        };
      }
    }
  }
}

/// A command's output as the model and Activity get it.
#[derive(Default)]
struct Output {
  head: String,
  /// Once the head is full, the latest lines, at most [`MODEL_HALF`] bytes.
  tail: VecDeque<String>,
  tail_bytes: usize,
  head_full: bool,
  total: usize,
  activity: String,
  activity_full: bool,
}

impl Output {
  fn push(&mut self, line: &str) {
    let bytes = line.len() + 1;
    self.total += bytes;
    if !self.head_full && self.head.len() + bytes <= MODEL_HALF {
      self.head.push_str(line);
      self.head.push('\n');
    } else {
      self.head_full = true;
      self.tail.push_back(line.to_owned());
      self.tail_bytes += bytes;
      while self.tail_bytes > MODEL_HALF {
        let Some(dropped) = self.tail.pop_front() else { break };
        self.tail_bytes -= dropped.len() + 1;
      }
    }
    if !self.activity_full && self.activity.len() + bytes <= ACTIVITY_LIMIT {
      self.activity.push_str(line);
      self.activity.push('\n');
    } else {
      self.activity_full = true;
    }
  }

  /// The call's outcome: its output with how it ended; `timed_out` is the limit it reached.
  fn outcome(self, exit: &ProcessExit, timed_out: Option<Duration>, took: Duration) -> ToolOutcome {
    let exit_code = match exit {
      ProcessExit::Exited { code } => code.map(i64::from),
      ProcessExit::Terminated | ProcessExit::WaitFailed { .. } => None,
    };
    let ending = match (timed_out, exit, exit_code) {
      (Some(limit), ..) => format!(
        "[timed out after {} s: the command and everything it started were ended]",
        limit.as_secs()
      ),
      (None, ProcessExit::WaitFailed { reason }, _) => format!("[the command's end could not be read: {reason}]"),
      (None, _, Some(code)) => format!("exit code {code} after {:.1} s", took.as_secs_f64()),
      (None, ..) => format!("ended without an exit code after {:.1} s", took.as_secs_f64()),
    };
    let omitted = self.total - self.head.len() - self.tail_bytes;
    let mut content = self.head;
    if omitted > 0 {
      content.push_str(&format!("[{omitted} bytes of output left out here]\n"));
    }
    for line in &self.tail {
      content.push_str(line);
      content.push('\n');
    }
    content.push_str(&ending);
    let mut activity = self.activity;
    if self.activity_full {
      activity.push_str("[output beyond 1 MiB was not kept]\n");
    }
    ToolOutcome {
      content,
      is_error: timed_out.is_some() || exit_code != Some(0),
      activity,
      exit_code,
      changes: Vec::new(),
      detail: None,
    }
  }
}
