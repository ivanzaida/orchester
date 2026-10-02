use orchester_core::connection::{
  mcp::McpServerProblem,
  stream::{AgentEvent, ClaudeStream, CommandStatus, OutputStream},
};

fn failed(server: Option<&str>, detail: Option<&str>) -> AgentEvent {
  AgentEvent::McpServer {
    server: server.map(str::to_owned),
    problem: McpServerProblem::FailedToStart {
      detail: detail.map(str::to_owned),
    },
  }
}

/// The `init` line Claude Code 2.1.89 printed with a server that started, a missing program and an
/// unreachable URL (2026-09-28, trimmed).
const INIT: &str = r#"{"type":"system","subtype":"init","cwd":"w","session_id":"a7","tools":["Read","mcp__good-one__echo"],"mcp_servers":[{"name":"good-one","status":"connected"},{"name":"bad_one","status":"failed"},{"name":"http-one","status":"needs-auth"}],"model":"claude-haiku-4-5-20251001"}"#;

#[test]
fn mcp_claude_reports_each_server_that_did_not_start() {
  let mut stream = ClaudeStream::default();
  assert_eq!(
    stream.parse(OutputStream::Stdout, INIT),
    vec![
      AgentEvent::SessionStarted {
        session: "a7".to_owned()
      },
      failed(Some("bad_one"), None),
      failed(Some("http-one"), Some("needs-auth")),
    ]
  );
}

#[test]
fn mcp_claude_signed_in_allow_rule_permits_the_probe_call() {
  let fixture = include_str!("fixtures/claude-signed-in-mcp.jsonl");
  let mut stream = ClaudeStream::default();
  let mut lines = fixture.lines();
  let init = stream.parse(OutputStream::Stdout, lines.next().expect("session init line"));
  assert_eq!(
    init,
    vec![AgentEvent::SessionStarted {
      session: "m1-signed-in".to_owned(),
    }]
  );
  let call = stream.parse(OutputStream::Stdout, lines.next().expect("MCP call line"));
  assert!(call.iter().any(|event| matches!(
    event,
    AgentEvent::CommandStarted { item, command }
      if item == "m1-tool" && command == "mcp__m1probe__ping"
  )));
  let result = stream.parse(OutputStream::Stdout, lines.next().expect("MCP result line"));
  assert_eq!(
    result,
    vec![AgentEvent::CommandFinished {
      item: "m1-tool".to_owned(),
      command: "mcp__m1probe__ping".to_owned(),
      output: "M1_TOOL_OK; server_secret_present=false".to_owned(),
      exit_code: None,
      status: CommandStatus::Completed,
    }]
  );
  let answer = stream.parse(OutputStream::Stdout, lines.next().expect("model answer line"));
  assert!(answer.iter().any(|event| matches!(
    event,
    AgentEvent::Message { text } if text.contains("M1_TOOL_OK")
  )));
  let done = stream.parse(OutputStream::Stdout, lines.next().expect("successful turn line"));
  // The probe asked for no step result, so its answer ends the turn without one.
  assert!(
    matches!(done.as_slice(), [AgentEvent::TurnFailed { .. }]),
    "the result repeats the already parsed model answer: {done:?}"
  );
  assert!(lines.next().is_none());
}
