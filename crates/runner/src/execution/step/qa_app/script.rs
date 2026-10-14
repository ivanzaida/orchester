//! Runs the worktree's `tools/qa-app.ps1` (`orchester_core::orchester_tools::QA_APP_SCRIPT`), which builds the app,
//! seeds its scenario and starts the copy off-screen without focus, then prints the copy's `pid` and `discovery`
//! lines: with Windows PowerShell on Windows, with PowerShell 7 (`pwsh`) on macOS, as the script supports both.

use std::{path::Path, time::Duration};

use orchester_core::{
  command::{Command, CommandError},
  orchester_tools::{OutputKept, ProgramOutput, QA_APP_SCRIPT},
};
use orchester_exec::run_capped;

/// How long building and starting the copy may take: a build of the app from a cold cache.
const START_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// How much of each stream is kept, from its end, where the script reports how it went.
const OUTPUT_LIMIT: usize = 16 * 1024;
/// How much of the output a refusal quotes.
const QUOTED: usize = 4 * 1024;

/// The script's output once it ended with exit code 0; why it did not otherwise.
pub(super) fn run(worktree: &Path, args: &[String]) -> Result<ProgramOutput, String> {
  let script = worktree.join(QA_APP_SCRIPT);
  if !script.is_file() {
    return Err(format!(
      "this worktree has no {QA_APP_SCRIPT}, so Orchester cannot start a QA app for it"
    ));
  }
  let (program, leading) = shell()?;
  let script = script.to_string_lossy().into_owned();
  let all = leading
    .iter()
    .map(|arg| (*arg).to_owned())
    .chain([script])
    .chain(args.iter().cloned());
  let command = Command::new(Path::new(program), all).with_current_dir(worktree);
  match run_capped(&command, START_TIMEOUT, (OUTPUT_LIMIT, OutputKept::Tail)) {
    Ok(output) if output.code == Some(0) => Ok(output),
    Ok(output) => Err(failure(&output)),
    Err(CommandError::NotFound) => Err(format!("{program} was not found, so {QA_APP_SCRIPT} cannot run")),
    Err(CommandError::TimedOut) => Err(format!(
      "{QA_APP_SCRIPT} did not finish within {} minutes and was ended",
      START_TIMEOUT.as_secs() / 60
    )),
    Err(error) => Err(format!("{QA_APP_SCRIPT} could not run: {error}")),
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
    "{QA_APP_SCRIPT} ended {code} without naming a started app. The end of its output:\n{}",
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
