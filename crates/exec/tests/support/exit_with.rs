//! Writes each of its arguments on a line of its own, as a Rust string literal, to the file `ORCHESTER_TEST_ARGS`
//! names, and exits with the code `ORCHESTER_TEST_EXIT_CODE` names (0 without one), so a test can see exactly what a
//! program was given, whatever a console between them does to its output, and how it ended.

use std::{env, fmt::Write, fs, io, process::ExitCode};

fn main() -> io::Result<ExitCode> {
  let mut text = String::new();
  for arg in env::args_os().skip(1) {
    writeln!(text, "{:?}", arg.to_string_lossy()).map_err(io::Error::other)?;
  }
  if let Some(file) = env::var_os("ORCHESTER_TEST_ARGS") {
    fs::write(file, text)?;
  }
  let code = env::var("ORCHESTER_TEST_EXIT_CODE")
    .ok()
    .and_then(|code| code.parse::<u8>().ok())
    .unwrap_or(0);
  Ok(ExitCode::from(code))
}
