//! A harmless program for the command tool tests. It reads its arguments as a list of actions and
//! carries them out in order:
//!
//! - `print <n>`: prints `line 1` … `line <n>` on standard output;
//! - `say <text>`: prints `<text>` on standard output at once;
//! - `stderr <text>`: prints `<text>` on standard error;
//! - `env <name>`: prints `<name>=<value>` of its environment, or `<name> is not set`;
//! - `write <file> <text>`: writes `<text>` to `<file>` in its working directory;
//! - `copy <from> <to>`: copies the file `<from>` to `<to>`, making the folders `<to>` needs;
//! - `cat <file>`: prints the file `<file>` on standard output;
//! - `pid-file <file>`: writes its own pid to `<file>`; `env-file <file> <name>` the value of `<name>`, or `<not set>`;
//! - `fail-once <file>`: exits with 1 after creating `<file>` in its working directory, unless the file exists;
//! - `spawn-sleeper`: starts a copy of itself that sleeps for ten minutes and prints `child pid: <pid>`;
//!   `spawn-sleeper-into <file>` writes the copy's pid to `<file>` instead;
//! - `listen <port>`: a stand-in web server on `127.0.0.1:<port>` until it is ended (`fake_server`);
//! - `connect <port>`: fails unless something accepts connections on `127.0.0.1:<port>`;
//! - `sleep <seconds>`: sleeps;
//! - `exit <code>`: exits with `<code>`.
//!
//! Without an `exit` it exits with 0. It prints `pid: <pid>` first.

#![allow(
  clippy::print_stdout,
  clippy::print_stderr,
  reason = "a stand-in for a program a tool runs: printing is what it is for"
)]

mod fake_run_app_server;
mod fake_server;

use std::{fs, io::Write, process::ExitCode, thread, time::Duration};

/// Set for the copy `spawn-sleeper` starts, which only sleeps.
const SLEEPER: &str = "ORCHESTER_TEST_COMMAND_SLEEPER";

fn main() -> ExitCode {
  if std::env::var_os(SLEEPER).is_some() {
    thread::sleep(Duration::from_secs(600));
    return ExitCode::SUCCESS;
  }
  println!("pid: {}", std::process::id());
  let args: Vec<String> = std::env::args().skip(1).collect();
  let mut rest = args.iter().map(String::as_str);
  while let Some(action) = rest.next() {
    let result = match action {
      "print" => print(rest.next()),
      "say" => say(rest.next()),
      "stderr" => {
        eprintln!("{}", rest.next().unwrap_or_default());
        Ok(())
      }
      "write" => write(rest.next(), rest.next()),
      "copy" => copy(rest.next(), rest.next()),
      "cat" => cat(rest.next()),
      "pid-file" => write(rest.next(), Some(&std::process::id().to_string())),
      "env-file" => {
        let file = rest.next();
        let value = std::env::var(rest.next().unwrap_or_default()).unwrap_or_else(|_| "<not set>".to_owned());
        write(file, Some(&value))
      }
      "env" => {
        let name = rest.next().unwrap_or_default();
        match std::env::var(name) {
          Ok(value) => println!("{name}={value}"),
          Err(_) => println!("{name} is not set"),
        }
        Ok(())
      }
      "fail-once" => match fail_once(rest.next()) {
        Ok(true) => return ExitCode::from(1),
        Ok(false) => Ok(()),
        Err(error) => Err(error),
      },
      "spawn-sleeper" => spawn_sleeper(None),
      "spawn-sleeper-into" => spawn_sleeper(rest.next()),
      "serve-app" => fake_run_app_server::serve(rest.next().unwrap_or_default()),
      "listen" => fake_server::listen(rest.next()),
      "connect" => fake_server::connect(rest.next()),
      "sleep" => sleep(rest.next()),
      "exit" => return ExitCode::from(rest.next().and_then(|code| code.parse().ok()).unwrap_or(2)),
      other => Err(format!("unknown action {other}")),
    };
    if let Err(error) = result {
      eprintln!("{error}");
      return ExitCode::from(2);
    }
  }
  ExitCode::SUCCESS
}

fn say(text: Option<&str>) -> Result<(), String> {
  let mut out = std::io::stdout().lock();
  writeln!(out, "{}", text.unwrap_or_default()).map_err(|error| error.to_string())?;
  out.flush().map_err(|error| error.to_string())
}

fn print(count: Option<&str>) -> Result<(), String> {
  let count: u32 = count
    .and_then(|count| count.parse().ok())
    .ok_or("print needs a count")?;
  let mut out = std::io::stdout().lock();
  for line in 1..=count {
    writeln!(out, "line {line}").map_err(|error| error.to_string())?;
  }
  Ok(())
}

fn write(file: Option<&str>, text: Option<&str>) -> Result<(), String> {
  let file = file.ok_or("write needs a file")?;
  fs::write(file, text.unwrap_or_default()).map_err(|error| error.to_string())
}

fn copy(from: Option<&str>, to: Option<&str>) -> Result<(), String> {
  let (from, to) = from.zip(to).ok_or("copy needs a file and where it goes")?;
  if let Some(folder) = std::path::Path::new(to).parent() {
    fs::create_dir_all(folder).map_err(|error| error.to_string())?;
  }
  fs::copy(from, to).map(|_| ()).map_err(|error| error.to_string())
}

fn cat(file: Option<&str>) -> Result<(), String> {
  let text = fs::read_to_string(file.ok_or("cat needs a file")?).map_err(|error| error.to_string())?;
  let mut out = std::io::stdout().lock();
  out.write_all(text.as_bytes()).map_err(|error| error.to_string())?;
  out.flush().map_err(|error| error.to_string())
}

/// `true` when this call created the file: the program fails this time.
fn fail_once(file: Option<&str>) -> Result<bool, String> {
  let file = file.ok_or("fail-once needs a file")?;
  if fs::metadata(file).is_ok() {
    return Ok(false);
  }
  fs::write(file, "failed once").map_err(|error| error.to_string())?;
  println!("failing once");
  Ok(true)
}

/// Starts the sleeping copy; its pid is printed, or written to `file` when one is named.
fn spawn_sleeper(file: Option<&str>) -> Result<(), String> {
  let program = std::env::current_exe().map_err(|error| error.to_string())?;
  let mut command = std::process::Command::new(program);
  command.env(SLEEPER, "1");
  #[cfg(windows)]
  if std::env::var_os("ORCHESTER_TEST_COMMAND_DETACHED").is_some() {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0000_0008);
  }
  let child = command.spawn().map_err(|error| error.to_string())?;
  if let Some(file) = file {
    return write(Some(file), Some(&child.id().to_string()));
  }
  println!("child pid: {}", child.id());
  std::io::stdout().flush().map_err(|error| error.to_string())
}

fn sleep(seconds: Option<&str>) -> Result<(), String> {
  let seconds: u64 = seconds
    .and_then(|seconds| seconds.parse().ok())
    .ok_or("sleep needs seconds")?;
  std::io::stdout().flush().map_err(|error| error.to_string())?;
  thread::sleep(Duration::from_secs(seconds));
  Ok(())
}
