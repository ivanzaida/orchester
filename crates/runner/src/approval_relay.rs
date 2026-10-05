//! `orchester-runner --claude-permission-prompt` and `orchester-runner --orchester-tools`: the MCP server
//! an agent CLI starts for one invocation. Claude Code starts it as its permission prompt
//! (`orchester_core::connection::claude_approval`), which also serves the step's Orchester tools
//! (`orchester_core::orchester_tools`); Codex starts it for the tools only. It reads the runner's loopback
//! address and the invocation's token from its environment, speaks MCP on standard input and output, and
//! hands each prompt call, tool list and tool call to the runner, which answers it. It holds no decision
//! of its own: a prompt call it cannot hand over is denied as unavailable ([`unavailable`]), never as a
//! person's denial, a tool call fails as unavailable, and the reason goes to standard error, which the CLI
//! keeps in its MCP log. The runner reports a prompt relay that never reached it
//! (`RunnerProblem::PermissionPromptUnreachable`).

use std::{
  env,
  io::{self, BufRead, BufReader, Write},
  net::TcpStream,
  process::ExitCode,
};

use orchester_core::{
  connection::claude_approval::{
    ADDRESS_VARIABLE, TOKEN_VARIABLE,
    relay::{RelayRequest, RelayStep, ask_result, call_result, list_result, relay_step},
    unavailable,
  },
  orchester_tools::ToolResult,
};
use serde_json::Value;

/// The argument that starts `orchester-runner` as Claude Code's permission prompt relay.
pub const RELAY_FLAG: &str = "--claude-permission-prompt";
/// The argument that starts it as the relay of the Orchester tools only.
pub const TOOLS_FLAG: &str = "--orchester-tools";
const TOOLS_UNAVAILABLE: &str = "Orchester's tools are unavailable: the runner cannot be reached.";

/// Relays until the CLI closes standard input; `prompt` when it serves Claude Code's permission prompt.
pub fn run(prompt: bool) -> ExitCode {
  let mut relay = Relay::connect(if prompt { RELAY_FLAG } else { TOOLS_FLAG });
  let mut output = io::stdout().lock();
  for line in io::stdin().lock().lines() {
    let Ok(line) = line else {
      return ExitCode::FAILURE;
    };
    let reply = match relay_step(&line, prompt) {
      RelayStep::Reply(reply) => reply,
      RelayStep::Ignore => continue,
      RelayStep::Ask { id, arguments } => {
        let decision = relay.exchange(&RelayRequest::Ask(arguments));
        ask_result(&id, &decision.unwrap_or_else(unavailable))
      }
      RelayStep::List { id } => {
        // A list that cannot be read lists no Orchester tool rather than failing the CLI's start.
        let tools = relay
          .exchange(&RelayRequest::List)
          .and_then(|answer| serde_json::from_str::<Vec<Value>>(&answer).ok())
          .unwrap_or_default();
        list_result(&id, tools, prompt)
      }
      RelayStep::Call { id, name, arguments } => {
        let result = relay
          .exchange(&RelayRequest::Call { name, arguments })
          .and_then(|answer| serde_json::from_str::<Value>(&answer).ok())
          .unwrap_or_else(|| ToolResult::unavailable(TOOLS_UNAVAILABLE).mcp());
        call_result(&id, &result)
      }
    };
    if writeln!(output, "{reply}").and_then(|()| output.flush()).is_err() {
      return ExitCode::FAILURE;
    }
  }
  ExitCode::SUCCESS
}

/// The relay's connection to the runner, while it holds.
struct Relay {
  flag: &'static str,
  runner: Option<Runner>,
}

#[allow(
  clippy::print_stderr,
  reason = "standard error is the relay's log, which the CLI keeps"
)]
impl Relay {
  fn connect(flag: &'static str) -> Self {
    let runner = Runner::connect()
      .inspect_err(|reason| eprintln!("{flag}: cannot reach Orchester: {reason}"))
      .ok();
    Self { flag, runner }
  }

  /// The runner's answer to `request`; `None` without a runner, or when the connection broke, which
  /// then stays closed.
  fn exchange(&mut self, request: &RelayRequest) -> Option<String> {
    match self.runner.as_mut()?.ask(request) {
      Ok(answer) => Some(answer),
      Err(reason) => {
        eprintln!("{}: lost Orchester: {reason}", self.flag);
        self.runner = None;
        None
      }
    }
  }
}

/// The connection to the runner that answers.
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

  /// The runner's answer to `request`, waiting as long as a person or the call takes, or why the
  /// connection broke.
  fn ask(&mut self, request: &RelayRequest) -> Result<String, String> {
    let line = serde_json::to_string(request).map_err(|error| error.to_string())?;
    writeln!(self.writer, "{line}").map_err(|error| error.to_string())?;
    let mut answer = String::new();
    match self.reader.read_line(&mut answer) {
      Ok(0) => Err("the connection closed".to_owned()),
      Ok(_) => Ok(answer.trim_end().to_owned()),
      Err(error) => Err(error.to_string()),
    }
  }
}
