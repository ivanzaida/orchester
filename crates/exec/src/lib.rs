//! Runs programs to completion for Orchester: the process side of
//! [`orchester_core::command::CommandRunner`]. Which programs run with which arguments is decided
//! in core; this starts the process, feeds its input, collects its output and enforces a
//! timeout. Everything here blocks; callers with a UI or an async runtime run it off their
//! thread.

mod capped;
mod console;
mod exchange;

pub mod disk_space;
pub mod external;
pub mod launch;
pub mod local_git;
pub mod priority;
#[cfg(unix)]
pub mod process_group;
pub mod programs;
pub mod ripgrep;
pub mod run_processes;
pub mod search_path;
pub mod sign_in_terminal;
pub mod trash;
pub mod verifier_app;

use std::{
  io::{self, Read, Write},
  process::{Child, ChildStdin, Command as Process, ExitStatus, Stdio},
  thread,
  time::{Duration, Instant},
};

use orchester_core::command::{Command, CommandError, CommandOutput, CommandRunner};

pub use self::capped::run_capped;

const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Runs programs as child processes, stopping any that does not finish within `timeout`.
#[derive(Clone, Copy, Debug)]
pub struct ProcessRunner {
  timeout: Duration,
}

impl ProcessRunner {
  pub const fn new(timeout: Duration) -> Self {
    Self { timeout }
  }
}

impl CommandRunner for ProcessRunner {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let mut child = spawn(command)?;
    // Feed and drain every pipe on its own thread while waiting: a full pipe would otherwise
    // block the program forever.
    let stdin = command
      .stdin()
      .map(|input| write_all(child.stdin.take(), input.to_owned()));
    let stdout = read_to_end(child.stdout.take());
    let stderr = read_to_end(child.stderr.take());
    let status = wait(&mut child, command, self.timeout)?;
    if let Some(writer) = stdin {
      joined_input(writer)?;
    }
    Ok(CommandOutput {
      code: status.code(),
      stdout: joined(stdout)?,
      stderr: joined(stderr)?,
    })
  }
}

fn spawn(command: &Command) -> Result<Child, CommandError> {
  prepare(command)?.spawn().map_err(|error| match error.kind() {
    io::ErrorKind::NotFound => CommandError::NotFound,
    _ => CommandError::Failed(error.to_string()),
  })
}

/// The process that runs `command`, ready to start: launched through what [`launch::resolve`]
/// finds, with its arguments, directory and environment, standard input piped when the command
/// has input (otherwise empty), output piped, no console window and, on macOS, leading a process
/// group of its own ([`process_group`]) with the full search path ([`search_path`]). Secret variables are
/// set last; the returned process holds their values, so it is not logged. For callers that stream a
/// program's output instead of waiting for it, such as the runner with agent CLIs.
pub fn prepare(command: &Command) -> Result<Process, CommandError> {
  let launch = launch::resolve(command.program());
  launch.check_args(command.args())?;
  let mut process = Process::new(launch.program());
  if !command.inherits_env() {
    process.env_clear();
  }
  process
    .args(launch.leading_args())
    .args(command.args())
    .envs(command.env().iter().map(|(key, value)| (key, value)))
    .envs(
      command
        .secret_env()
        .iter()
        .map(|variable| (variable.name(), variable.expose())),
    )
    .stdin(if command.stdin().is_some() || command.interactive_stdin() {
      Stdio::piped()
    } else {
      Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  for key in command.env_removed() {
    process.env_remove(key);
  }
  if let Some(directory) = command.current_dir() {
    process.current_dir(directory);
  }
  console::hide_window(&mut process);
  #[cfg(unix)]
  process_group::lead_own_group(&mut process);
  #[cfg(target_os = "macos")]
  search_path::macos::apply(&mut process, command, launch.program());
  Ok(process)
}

fn wait(child: &mut Child, command: &Command, timeout: Duration) -> Result<ExitStatus, CommandError> {
  let deadline = Instant::now() + timeout;
  loop {
    match child.try_wait() {
      Ok(Some(status)) => return Ok(status),
      Ok(None) if Instant::now() >= deadline => {
        if let Err(error) = stop(child) {
          tracing::warn!(%error, program = %command.program().display(), "cannot stop a program that did not finish");
        }
        return Err(CommandError::TimedOut);
      }
      Ok(None) => thread::sleep(POLL_INTERVAL),
      Err(error) => return Err(CommandError::Failed(error.to_string())),
    }
  }
}

/// Ends a program that ran too long.
#[cfg(windows)]
pub(crate) fn stop(child: &mut Child) -> io::Result<()> {
  child.kill()
}

/// Ends a program that ran too long with everything it started, and reaps it.
#[cfg(unix)]
pub(crate) fn stop(child: &mut Child) -> io::Result<()> {
  let group = process_group::ProcessGroup::led_by(child.id())
    .ok_or_else(|| io::Error::other(format!("invalid process id {}", child.id())))?;
  group.kill()?;
  child.wait().map(drop)
}

fn write_all(pipe: Option<ChildStdin>, input: String) -> thread::JoinHandle<io::Result<()>> {
  thread::spawn(move || {
    // Dropping the pipe at the end closes it, so the program sees the end of its input.
    let Some(mut pipe) = pipe else {
      return Ok(());
    };
    pipe.write_all(input.as_bytes())
  })
}

fn joined_input(writer: thread::JoinHandle<io::Result<()>>) -> Result<(), CommandError> {
  match writer
    .join()
    .map_err(|_| CommandError::Failed("the input writer panicked".into()))?
  {
    // The program exited without reading all of its input; its exit status and output say
    // how it went, so the unread rest is not an error of the run.
    Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
    result => result.map_err(|error| CommandError::Failed(error.to_string())),
  }
}

fn read_to_end(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<io::Result<String>> {
  thread::spawn(move || {
    let mut bytes = Vec::new();
    if let Some(mut pipe) = pipe {
      pipe.read_to_end(&mut bytes)?;
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
  })
}

fn joined(reader: thread::JoinHandle<io::Result<String>>) -> Result<String, CommandError> {
  reader
    .join()
    .map_err(|_| CommandError::Failed("the output reader panicked".into()))?
    .map_err(|error| CommandError::Failed(error.to_string()))
}
