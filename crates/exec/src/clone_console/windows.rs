//! The clone in a PowerShell console window of its own. The script is fixed text: the program, the address, the folder,
//! the result file and the window's words reach it only through environment variables, so nothing a server or a
//! person wrote is ever read as PowerShell, and PowerShell hands each one to Git as one argument.

use std::{
  env,
  os::windows::process::CommandExt,
  path::{Path, PathBuf},
  process::{Child, Command},
};

use super::{CloneJob, ConsoleEnd, ConsoleError, Scratch, follow};

/// The console program gets a console window of its own instead of none.
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// Runs the clone, writes Git's exit code beside the result and renames it there, and after a failure waits for Enter
/// so its message stays readable. One line without double quotes: `powershell.exe` takes it as one argument.
const SCRIPT: &str = "$Host.UI.RawUI.WindowTitle = $env:ORCHESTER_CLONE_TITLE; \
  $code = 1; \
  try { & $env:ORCHESTER_CLONE_GIT 'clone' '--' $env:ORCHESTER_CLONE_URL $env:ORCHESTER_CLONE_DESTINATION; \
  $code = $LASTEXITCODE } catch { Write-Host $_ }; \
  $written = $env:ORCHESTER_CLONE_RESULT + '.tmp'; \
  [IO.File]::WriteAllText($written, [string]$code); \
  Move-Item -LiteralPath $written -Destination $env:ORCHESTER_CLONE_RESULT; \
  if ($code -ne 0) { Write-Host ''; Write-Host $env:ORCHESTER_CLONE_FAILED; \
  [void](Read-Host $env:ORCHESTER_CLONE_CLOSE) }";

#[derive(Debug)]
pub(super) struct Running {
  host: Child,
  result: PathBuf,
}

pub(super) fn open(job: &CloneJob<'_>, files: &Scratch) -> Result<Running, ConsoleError> {
  let result = files.result();
  let mut process = command(job, &result);
  process.creation_flags(CREATE_NEW_CONSOLE);
  let host = process.spawn().map_err(ConsoleError::NotOpened)?;
  Ok(Running { host, result })
}

pub(super) fn wait(mut running: Running, _files: &Scratch) -> Result<ConsoleEnd, ConsoleError> {
  let host = &mut running.host;
  follow(&running.result, || {
    host.try_wait().map(|ended| ended.is_none()).map_err(ConsoleError::Wait)
  })
}

/// The PowerShell process that runs `job` and writes Git's exit code to `result`, without its window: [`open`] gives
/// it a console of its own, and a test one that is hidden. It inherits the app's environment but the two variables
/// Orchester's own Git work sets to keep Git from asking, so Git may ask for a sign-in here.
pub fn command(job: &CloneJob<'_>, result: &Path) -> Command {
  let mut process = Command::new(powershell());
  process
    .args(["-NoLogo", "-NoProfile", "-Command", SCRIPT])
    .env("ORCHESTER_CLONE_GIT", job.git)
    .env("ORCHESTER_CLONE_URL", job.url)
    .env("ORCHESTER_CLONE_DESTINATION", job.destination)
    .env("ORCHESTER_CLONE_RESULT", result)
    .env("ORCHESTER_CLONE_TITLE", job.words.title())
    .env("ORCHESTER_CLONE_FAILED", job.words.failed())
    .env("ORCHESTER_CLONE_CLOSE", job.words.close())
    .env_remove("GIT_TERMINAL_PROMPT")
    .env_remove("GCM_INTERACTIVE");
  process
}

/// Windows PowerShell by its full path in the system folder, so no `powershell.exe` elsewhere on the `PATH` runs.
fn powershell() -> PathBuf {
  let system = env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
  system.join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
}
