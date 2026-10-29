//! Finding the browser on Windows: Microsoft Edge, installed with Windows, then Google Chrome; each from the registry's
//! `App Paths`, the machine's and then the person's, then from its usual install folders. The registry is read with
//! `%SystemRoot%\System32\reg.exe`, named by its full path, so no `unsafe` registry call is needed.

use std::{env, path::PathBuf, time::Duration};

use orchester_core::command::{Command, CommandRunner};

use super::{BrowserProgram, BrowserUnavailable, first_installed, registry_default_value};
use crate::ProcessRunner;

/// Each browser's name, its program's file name, and where its installer puts it below an install folder.
const BROWSERS: [(&str, &str, &str); 2] = [
  ("Microsoft Edge", "msedge.exe", r"Microsoft\Edge\Application\msedge.exe"),
  ("Google Chrome", "chrome.exe", r"Google\Chrome\Application\chrome.exe"),
];
/// The variables naming the install folders: for all users (32-bit, then 64-bit), then for the person only.
const INSTALL_FOLDERS: [&str; 3] = ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"];
/// The registry hives whose `App Paths` name an installed program: the machine's, then the person's.
const HIVES: [&str; 2] = ["HKLM", "HKCU"];
/// How long `reg.exe` may take to answer.
const REG_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn find() -> Result<BrowserProgram, BrowserUnavailable> {
  let mut candidates = Vec::new();
  for (name, file, below) in BROWSERS {
    candidates.extend(
      HIVES
        .iter()
        .filter_map(|hive| app_path(hive, file).map(|path| (name, path))),
    );
    candidates.extend(
      INSTALL_FOLDERS
        .iter()
        .filter_map(|variable| env::var_os(variable).map(|folder| (name, PathBuf::from(folder).join(below)))),
    );
  }
  first_installed(
    candidates,
    "the registry's App Paths, Program Files and the local application data",
  )
}

/// The program `hive`'s `App Paths` name for `file`; `None` when the key is missing or cannot be read, which leaves
/// the install folders to look in.
fn app_path(hive: &str, file: &str) -> Option<PathBuf> {
  let system_root = env::var_os("SystemRoot")?;
  let reg = PathBuf::from(system_root).join("System32").join("reg.exe");
  let key = format!(r"{hive}\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{file}");
  let output = ProcessRunner::new(REG_TIMEOUT)
    .run(&Command::new(&reg, ["query", key.as_str(), "/ve"]))
    .inspect_err(|error| tracing::warn!(%error, %key, "cannot read the registry for the browser"))
    // The install folders are still looked in; the failure is logged.
    .ok()?;
  output
    .success()
    .then(|| registry_default_value(&output.stdout))
    .flatten()
}
