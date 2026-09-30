//! `orchester-runner --claude-permission-prompt`: the MCP server Claude Code starts as its
//! permission prompt (`orchester_core::connection::claude_approval`). It reads the runner's
//! loopback address and the invocation's token from its environment, speaks MCP on standard input
//! and output, and hands each call to the runner, which answers it with a person's decision. It
//! holds no decision of its own: a call it cannot hand over is denied as unavailable
//! ([`unavailable`]), never as a person's denial, and the reason goes to standard error, which
//! Claude Code keeps in its MCP log. The runner reports a relay that never reached it
//! (`RunnerProblem::PermissionPromptUnreachable`).

use std::{
  env,
  io::{self, BufRead, BufReader, Write},
  net::TcpStream,
  process::ExitCode,
};

use orchester_core::connection::claude_approval::{
  ADDRESS_VARIABLE, TOKEN_VARIABLE,
  relay::{RelayStep, ask_result, relay_step},
  unavailable,
};
use serde_json::Value;

/// The argument that starts `orchester-runner` as the relay.
pub const RELAY_FLAG: &str = "--claude-permission-prompt";

/// Relays until Claude Code closes standard input.
#[allow(
  clippy::print_stderr,
  reason = "standard error is the relay's log, which Claude Code keeps"
)]
pub fn run() -> ExitCode {
  let mut runner = Runner::connect()
    .inspect_err(|reason| eprintln!("{RELAY_FLAG}: cannot reach Orchester: {reason}"))
    .ok();
  let mut output = io::stdout().lock();
  for line in io::stdin().lock().lines() {
    let Ok(line) = line else {
      return ExitCode::FAILURE;
    };
    let reply = match relay_step(&line) {
      RelayStep::Reply(reply) => reply,
      RelayStep::Ask { id, arguments } => {
        let answer = runner.as_mut().map(|connected| connected.ask(&arguments));
        let decision = match answer {
          Some(Ok(decision)) => decision,
          Some(Err(reason)) => {
            eprintln!("{RELAY_FLAG}: lost Orchester: {reason}");
            runner = None;
            unavailable()
          }
          None => unavailable(),
        };
        ask_result(&id, &decision)
      }
      RelayStep::Ignore => continue,
    };
    if writeln!(output, "{reply}").and_then(|()| output.flush()).is_err() {
      return ExitCode::FAILURE;
    }
  }
  ExitCode::SUCCESS
}

/// The connection to the runner that waits for the decisions.
struct Runner {
  reader: BufReader<TcpStream>,
  writer: TcpStream,
}

impl Runner {
  /// Connects and shows the token, or why the runner cannot be reached.
  fn connect() -> Result<Self, String> {
    let address = env::var(ADDRESS_VARIABLE).map_err(|error| format!("{ADDRESS_VARIABLE}: {error}"))?;
    let token = env::var(TOKEN_VARIABLE).map_err(|error| format!("{TOKEN_VARIABLE}: {error}"))?;
    let stream = TcpStream::connect(&address).map_err(|error| format!("{address}: {error}"))?;
    let mut writer = stream.try_clone().map_err(|error| error.to_string())?;
    writeln!(writer, "{token}").map_err(|error| error.to_string())?;
    Ok(Self {
      reader: BufReader::new(stream),
      writer,
    })
  }

  /// The runner's answer to the call with `arguments`, waiting as long as the person takes, or
  /// why the connection broke.
  fn ask(&mut self, arguments: &Value) -> Result<String, String> {
    writeln!(self.writer, "{arguments}").map_err(|error| error.to_string())?;
    let mut answer = String::new();
    match self.reader.read_line(&mut answer) {
      Ok(0) => Err("the connection closed".to_owned()),
      Ok(_) => Ok(answer.trim_end().to_owned()),
      Err(error) => Err(error.to_string()),
    }
  }
}
