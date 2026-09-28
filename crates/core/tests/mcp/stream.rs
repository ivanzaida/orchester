use orchester_core::connection::{
  AgentProvider,
  mcp::McpServerProblem,
  stream::{AgentEvent, AgentStream, InvocationTracker, OutputStream, parse_codex_line},
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
