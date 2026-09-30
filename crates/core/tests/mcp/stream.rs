use orchester_core::connection::{
  AgentProvider,
  mcp::McpServerProblem,
  stream::{AgentEvent, AgentStream, CommandStatus, InvocationTracker, OutputStream, parse_codex_line},
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
  let mut stream = AgentStream::new(AgentProvider::ClaudeCodeCli);
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
  let mut stream = AgentStream::new(AgentProvider::ClaudeCodeCli);
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

#[test]
fn mcp_codex_reports_a_server_that_did_not_start_without_failing_the_turn() {
  let named = r#"{"type":"error","message":"MCP client for `docs` failed to start: program not found"}"#;
  let event = parse_codex_line(OutputStream::Stdout, named);
  assert_eq!(event, failed(Some("docs"), Some("program not found")));
  let logged = "2026-09-28T11:01:24.796533Z  WARN session_loop{thread_id=01a0}: codex_mcp::rmcp_client: failed to \
                initialize MCP client during shutdown: MCP startup failed: program not found";
  assert_eq!(
    parse_codex_line(OutputStream::Stderr, logged),
    failed(None, Some("program not found"))
  );
  let mut tracker = InvocationTracker::default();
  tracker.observe(&event);
  tracker.observe(&AgentEvent::TurnCompleted {
    usage: Default::default(),
  });
  assert!(
    tracker.outcome(Some(0)).is_success(),
    "a server that did not start is not the turn's failure"
  );
  let other = r#"{"type":"error","message":"Reconnecting... 2/5"}"#;
  assert!(matches!(
    parse_codex_line(OutputStream::Stdout, other),
    AgentEvent::Error { .. }
  ));
}

#[test]
fn mcp_codex_signed_in_startup_fixture_is_an_unnamed_server_problem() {
  let warning = include_str!("fixtures/codex-signed-in-startup.stderr").trim();
  assert_eq!(
    parse_codex_line(OutputStream::Stderr, warning),
    failed(None, Some("The system cannot find the file specified. (os error 2)"))
  );
  let completed = include_str!("fixtures/codex-signed-in-startup.jsonl").trim();
  assert!(matches!(
    parse_codex_line(OutputStream::Stdout, completed),
    AgentEvent::TurnCompleted { .. }
  ));
}

#[test]
fn mcp_codex_signed_in_tool_fixture_preserves_the_call_and_excluded_shell_result() {
  let fixture = include_str!("fixtures/codex-signed-in-mcp.jsonl");
  let mut lines = fixture.lines();
  let call = lines.next().expect("MCP tool call line");
  assert!(matches!(
    parse_codex_line(OutputStream::Stdout, call),
    AgentEvent::Raw {
      stream: OutputStream::Stdout,
      ..
    }
  ));
  let shell = lines.next().expect("shell result line");
  assert!(matches!(
    parse_codex_line(OutputStream::Stdout, shell),
    AgentEvent::CommandFinished {
      output,
      exit_code: Some(0),
      status: CommandStatus::Completed,
      ..
    } if output.trim() == "False"
  ));
  assert!(lines.next().is_none());
}
