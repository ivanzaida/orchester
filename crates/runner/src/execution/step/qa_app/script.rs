//! Runs the worktree's `tools/qa-app.ps1` (`orchester_core::orchester_tools::QA_APP_SCRIPT`), which builds the app,
//! seeds its scenario and starts the copy off-screen without focus, then prints the copy's `pid` and `discovery`
//! lines: with Windows PowerShell on Windows, with PowerShell 7 (`pwsh`) on macOS, as the script supports both.
//!
//! The script runs in a process tree of its own ([`Tree`]) and is watched while it runs: when its turn ends or its
//! run stops (`ended`), or it runs longer than [`START_TIMEOUT`], it is ended with everything it started. A script
//! that ran to its end hands back its output with the tree ([`Ran`]): the caller keeps the tree's app running, or
//! drops it, which ends what the script left behind.

use std::{
  collections::VecDeque,
  io::{self, Read},
  path::Path,
  process::Child,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  command::{Command, CommandError},
  orchester_tools::{ProgramOutput, QA_APP_SCRIPT},
};

use super::tree::Tree;

/// How long building and starting the copy may take: a build of the app from a cold cache.
const START_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// How often the script is looked at while it runs.
const POLL: Duration = Duration::from_millis(200);
/// How much of each stream is kept, from its end, where the script reports how it went.
const OUTPUT_LIMIT: usize = 16 * 1024;
/// How much of the output a refusal quotes.
const QUOTED: usize = 4 * 1024;

/// A script that ran to its end with exit code 0, and what it left running. Dropping it ends that.
pub(super) struct Ran {
  pub(super) output: ProgramOutput,
  tree: Tree,
}

impl Ran {
  /// Lets the app the script started outlive it.
  pub(super) fn keep(self) -> Result<(), String> {
    self
      .tree
      .keep()
      .map_err(|error| format!("the QA app cannot be kept running: {error}"))
  }
}

/// Runs the script with `args` in `worktree` until it ends, `ended` says to give up, or it times out.
pub(super) fn run(worktree: &Path, args: &[String], ended: &dyn Fn() -> bool) -> Result<Ran, String> {
  let script = worktree.join(QA_APP_SCRIPT);
  if !script.is_file() {
    return Err(format!(
      "this worktree has no {QA_APP_SCRIPT}, so Orchester cannot start a QA app for it"
    ));
  }
  let (program, leading) = shell()?;
  let all = leading
    .iter()
    .map(|arg| (*arg).to_owned())
    .chain([script.to_string_lossy().into_owned()])
    .chain(args.iter().cloned());
  let command = Command::new(Path::new(program), all).with_current_dir(worktree);
  let started = orchester_exec::prepare(&command).and_then(|mut process| {
    process.spawn().map_err(|error| match error.kind() {
      io::ErrorKind::NotFound => CommandError::NotFound,
      _ => CommandError::Failed(error.to_string()),
    })
  });
  let mut child = match started {
    Ok(child) => child,
    Err(CommandError::NotFound) => return Err(format!("{program} was not found, so {QA_APP_SCRIPT} cannot run")),
    Err(error) => return Err(format!("{QA_APP_SCRIPT} could not run: {error}")),
  };
  let tree = match Tree::of(&child) {
    Ok(tree) => tree,
    Err(error) => {
      end_alone(&mut child);
      return Err(format!(
        "{QA_APP_SCRIPT} could not be kept together with what it starts: {error}"
      ));
    }
  };
  watch(child, tree, ended)
}

/// Waits for the script to end and reads what it printed; ends its tree when it is given up.
fn watch(mut child: Child, tree: Tree, ended: &dyn Fn() -> bool) -> Result<Ran, String> {
  let stdout = tail(child.stdout.take());
  let stderr = tail(child.stderr.take());
  let deadline = Instant::now() + START_TIMEOUT;
  let status = loop {
    match child.try_wait() {
      Ok(Some(status)) => break Ok(status),
      Ok(None) if ended() => {
        break Err(
          "the turn ended while the QA app was being built and started; the script and what it started were ended"
            .to_owned(),
        );
      }
      Ok(None) if Instant::now() >= deadline => {
        break Err(format!(
          "{QA_APP_SCRIPT} did not finish within {} minutes and was ended with what it started",
          START_TIMEOUT.as_secs() / 60
        ));
      }
      Ok(None) => thread::sleep(POLL),
      Err(error) => break Err(format!("{QA_APP_SCRIPT} could not be watched: {error}")),
    }
  };
  let status = match status {
    Ok(status) => status,
    Err(refusal) => {
      // Ending the tree ends the script too, which closes its pipes for the readers.
      drop(tree);
      end_alone(&mut child);
      return Err(refusal);
    }
  };
  let (stdout, stdout_left) = joined(stdout);
  let (stderr, stderr_left) = joined(stderr);
  let output = ProgramOutput {
    code: status.code(),
    stdout,
    stderr,
    left_out: stdout_left + stderr_left,
  };
  if output.code != Some(0) {
    // Dropping the tree ends whatever the failed script left running.
    return Err(failure(&output));
  }
  Ok(Ran { output, tree })
}

/// Ends the script itself and reaps it; a script that already exited needs nothing.
fn end_alone(child: &mut Child) {
  if let Err(error) = child.kill() {
    tracing::debug!(%error, "the QA script had already ended");
  }
  if let Err(error) = child.wait() {
    tracing::warn!(%error, "the QA script's end could not be confirmed");
  }
}

/// Reads `pipe` to its end on a thread of its own, keeping its last [`OUTPUT_LIMIT`] bytes, and how many it did not.
fn tail(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<(Vec<u8>, u64)> {
  thread::spawn(move || {
    let mut kept = VecDeque::new();
    let mut left_out = 0_u64;
    let Some(mut pipe) = pipe else {
      return (Vec::new(), 0);
    };
    let mut chunk = [0_u8; 8 * 1024];
    loop {
      match pipe.read(&mut chunk) {
        // A pipe that cannot be read ends the output as its closing does; the exit code says how the script went.
        Ok(0) | Err(_) => break,
        Ok(read) => {
          kept.extend(&chunk[..read]);
          while kept.len() > OUTPUT_LIMIT {
            kept.pop_front();
            left_out += 1;
          }
        }
      }
    }
    (kept.into_iter().collect(), left_out)
  })
}

fn joined(reader: thread::JoinHandle<(Vec<u8>, u64)>) -> (String, u64) {
  match reader.join() {
    Ok((bytes, left_out)) => (String::from_utf8_lossy(&bytes).into_owned(), left_out),
    // A reader that panicked kept nothing to show.
    Err(_) => (String::new(), 0),
  }
}

/// Why the script started no app, with the end of what it printed.
pub(super) fn failure(output: &ProgramOutput) -> String {
  let code = output.code.map_or_else(
    || "without an exit code".to_owned(),
    |code| format!("with exit code {code}"),
  );
  let printed = format!("{}\n{}", output.stdout.trim_end(), output.stderr.trim_end());
  let printed = printed.trim();
  let mut start = printed.len().saturating_sub(QUOTED);
  while !printed.is_char_boundary(start) {
    start += 1;
  }
  format!(
    "{QA_APP_SCRIPT} ended {code} without naming a started app; what it started was ended. The end of its output:\n{}",
    &printed[start..]
  )
}

/// The program that runs the script and its arguments before the script's path.
#[cfg(windows)]
fn shell() -> Result<(&'static str, &'static [&'static str]), String> {
  Ok((
    "powershell.exe",
    &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"],
  ))
}

#[cfg(target_os = "macos")]
fn shell() -> Result<(&'static str, &'static [&'static str]), String> {
  Ok(("pwsh", &["-NoProfile", "-NonInteractive", "-File"]))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn shell() -> Result<(&'static str, &'static [&'static str]), String> {
  Err("QA apps are not supported on this platform yet".to_owned())
}
