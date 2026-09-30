//! `orchester-runner --claude-permission-prompt`: the MCP server Claude Code starts as its
//! permission prompt (`orchester_core::connection::claude_approval`). It reads the runner's
//! loopback address and the invocation's token from its environment, speaks MCP on standard input
//! and output, and hands each call to the runner, which answers it with a person's decision. It
//! holds no decision of its own: a call it cannot hand over is denied.

use std::{
  env,
  io::{self, BufRead, BufReader, Write},
  net::TcpStream,
  process::ExitCode,
};

use orchester_core::connection::claude_approval::{
  ADDRESS_VARIABLE, TOKEN_VARIABLE, prompt_result,
  relay::{RelayStep, ask_result, relay_step},
};
use serde_json::Value;

/// The argument that starts `orchester-runner` as the relay.
pub const RELAY_FLAG: &str = "--claude-permission-prompt";

/// Relays until Claude Code closes standard input.
pub fn run() -> ExitCode {
  let mut runner = Runner::connect();
  let mut output = io::stdout().lock();
  for line in io::stdin().lock().lines() {
    let Ok(line) = line else {
      return ExitCode::FAILURE;
    };
    let reply = match relay_step(&line) {
      RelayStep::Reply(reply) => reply,
      RelayStep::Ask { id, arguments } => {
        let decision = runner
          .as_mut()
          .and_then(|connected| connected.ask(&arguments))
          .unwrap_or_else(|| prompt_result(false));
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
  /// Connects and shows the token; `None` when the runner cannot be reached, which denies every call.
  fn connect() -> Option<Self> {
    let address = env::var(ADDRESS_VARIABLE).ok()?;
    let token = env::var(TOKEN_VARIABLE).ok()?;
    let stream = TcpStream::connect(address).ok()?;
    let mut writer = stream.try_clone().ok()?;
    writeln!(writer, "{token}").ok()?;
    Some(Self {
      reader: BufReader::new(stream),
      writer,
    })
  }

  /// The runner's answer to the call with `arguments`, waiting as long as the person takes;
  /// `None` when the connection broke.
  fn ask(&mut self, arguments: &Value) -> Option<String> {
    writeln!(self.writer, "{arguments}").ok()?;
    let mut answer = String::new();
    match self.reader.read_line(&mut answer) {
      Ok(0) | Err(_) => None,
      Ok(_) => Some(answer.trim_end().to_owned()),
    }
  }
}
