//! npm launchers of packages whose command is a program file rather than a Node.js script: since
//! Claude Code 2.1, npm's `cmd-shim` writes `claude.cmd` as its usual preamble followed by
//! `"%dp0%\node_modules\…\claude.exe" %*`. Such a launcher only hands its arguments to the
//! program, so the program can start in its place. Only that exact shape is recognised: a known
//! preamble, then one launch line, and nothing after it.

use std::{
  ffi::OsStr,
  path::{Path, PathBuf},
};

use super::command_script::inside_folder;

/// The preamble `cmd-shim` writes; it sets `dp0` to the launcher's folder.
const CMD_SHIM_PREAMBLE: [&str; 8] = [
  "@ECHO off",
  "GOTO start",
  ":find_dp0",
  "SET dp0=%~dp0",
  "EXIT /b",
  ":start",
  "SETLOCAL",
  "CALL :find_dp0",
];
/// Lines that neither run anything nor change where the script goes; a launcher that names its
/// folder as `%~dp0` needs no more.
const QUIET_LINES: [&str; 3] = ["@ECHO off", "@SETLOCAL", "SETLOCAL"];
/// The launcher's folder as `cmd-shim` names it after its preamble, and as `cmd.exe` itself
/// names it.
const DP0_FOLDER: &str = "%dp0%\\";
const SCRIPT_FOLDER: &str = "%~dp0\\";
const PROGRAM_EXTENSION: &str = "exe";

/// The program, relative to the launcher's folder, that an npm launcher starts with its own
/// arguments. `None` for any other script.
pub fn native_shim_program(text: &str) -> Option<PathBuf> {
  // Lines are compared as written: `cmd.exe` keeps trailing spaces, so `SET dp0=%~dp0 ` would set
  // a different folder.
  let lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
  let (launch, preamble) = lines.split_last()?;
  let (folder, program) = launch_line(launch)?;
  let preamble_fits = match folder {
    DP0_FOLDER => same_lines(preamble, &CMD_SHIM_PREAMBLE),
    _ => preamble
      .iter()
      .all(|line| QUIET_LINES.iter().any(|quiet| quiet.eq_ignore_ascii_case(line))),
  };
  if !preamble_fits {
    return None;
  }
  inside_folder(program)
}

/// `"<folder>\<program>.exe" %*`: how the launcher names its folder, and the program's relative
/// path.
fn launch_line(line: &str) -> Option<(&'static str, &str)> {
  let quoted = line
    .strip_suffix("%*")?
    .trim_end()
    .strip_prefix('"')?
    .strip_suffix('"')?;
  let (folder, program) = [DP0_FOLDER, SCRIPT_FOLDER]
    .into_iter()
    .find_map(|folder| Some((folder, quoted.strip_prefix(folder)?)))?;
  let is_program = Path::new(program)
    .extension()
    .and_then(OsStr::to_str)
    .is_some_and(|extension| extension.eq_ignore_ascii_case(PROGRAM_EXTENSION));
  // `%` would be expanded by `cmd.exe`, so the path would not be the one written here.
  (is_program && !program.contains(['%', '"'])).then_some((folder, program))
}

/// `cmd.exe` reads commands and labels without regard to case.
fn same_lines(lines: &[&str], expected: &[&str]) -> bool {
  lines.len() == expected.len()
    && lines
      .iter()
      .zip(expected)
      .all(|(line, expected)| line.eq_ignore_ascii_case(expected))
}
