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

/// The relays of Orchester's tools that did not come are stored as counts, which the desktop words; a run
/// recorded before then, with the runner's English sentence as the detail, still reads.
#[test]
fn mcp_missing_tool_relays_are_stored_as_counts_and_old_records_still_read() {
  let missing = AgentEvent::McpServer {
    server: Some("orchester".to_owned()),
    problem: McpServerProblem::ToolRelaysMissing { reached: 1, started: 2 },
  };
  let stored = serde_json::to_string(&missing).expect("an event serializes");
  assert_eq!(
    stored,
    r#"{"kind":"mcp_server","server":"orchester","problem":{"problem":"tool_relays_missing","reached":1,"started":2}}"#
  );
  assert_eq!(
    serde_json::from_str::<AgentEvent>(&stored).expect("it reads back"),
    missing
  );

  let old = r#"{"kind":"mcp_server","server":"orchester","problem":{"problem":"failed_to_start","detail":"Codex reached Orchester's tools 1 of the 2 times it starts them; the agent may have worked without them"}}"#;
  assert_eq!(
    serde_json::from_str::<AgentEvent>(old).expect("an old record reads"),
    AgentEvent::McpServer {
      server: Some("orchester".to_owned()),
      problem: McpServerProblem::FailedToStart {
        detail: Some(
          "Codex reached Orchester's tools 1 of the 2 times it starts them; the agent may have worked without them"
            .to_owned()
        ),
      },
    }
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
