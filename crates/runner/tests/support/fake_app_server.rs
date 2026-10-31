//! The requests of `orchester-fake-codex` that wait for a decision, and what each answer does. Side
//! effects happen only after the exact one-action answer.
//!
//! - In a resumed session, "Continue with approvals" asks about the command `disposable-command` (writing
//!   `command-once.txt` once approved), then about the `write` tool of the `disposable` MCP server (writing
//!   `mcp-once.txt`), and ends with the result "continued". "Hold approved command" and "Hold approved MCP" keep the
//!   approved action running until the fake is ended; "Release approved command" runs it until `release-active.txt`
//!   lies in the worktree.
//! - "Escalate `<command>`" wants to run that command outside the sandbox (`sandbox_permissions: require_escalated`,
//!   with the model's justification as the reason) and asks; approved, it appends `escalated` to `README.md` and works
//!   on, declined it reports the command declined, appends `went on` and works on, as Codex tells its model and the
//!   turn continues. Each answer is reported on standard error (`escalation: <decision>`).
//! - "Patch outside" wants to change a file outside the writable roots and asks; it reports the answer (`file change:
//!   <decision>`) and works on.
//! - "Use the design tools" calls the `get_screenshot` tool of the `pencil` MCP server (writing `read-once.txt`), then
//!   its `batch_design` tool as the `write` tool above, by the name Codex's arguments give the server. A tool the
//!   thread's configuration approves runs without a request; any other asks.
//! - "Call a tool unasked" calls `batch_design` of `pencil` and asks, as Codex 0.159.2 does even with
//!   `mcp_elicitations` off; it reports the answer (`mcp answer: <action>`), and when declined ends the call failed and
//!   works on.
//! - "Design twice" calls `batch_design` of `pencil` twice in one turn, each asking; each approved call appends
//!   `approved` to `README.md`. A declined MCP call (here or in the fixture) ends failed, appends `went on`, and the
//!   turn works on.
//! - "Ask Codex with options" asks two questions with suggested answers (`fake_codex_question`).

use std::{fs, process::ExitCode};

use serde_json::{Value, json};

use super::{
  Session, Step, complete, fake_codex_question,
  fake_codex_user::argument_name,
  fake_codex_work::{append, finish, message, report_of},
  notify, request,
};

/// The request ids of the fixture's command and MCP call, the escalation and the patch.
const COMMAND: u64 = 100;
const MCP: u64 = 101;
const ESCALATION: u64 = 200;
const PATCH: u64 = 300;
/// The second call of "Design twice".
const SECOND_MCP: u64 = 102;

/// The decisions a turn waits on.
pub struct Approvals {
  prompt: String,
  structured: bool,
  hold_approved: bool,
  hold_mcp: bool,
  release_command: bool,
}

/// Starts the requests `prompt` asks for, if any.
pub fn start(session: &mut Session, prompt: &str, structured: bool) -> Option<Step> {
  let fixture = session.resumed
    && [
      "Continue with approvals",
      "Hold approved command",
      "Hold approved MCP",
      "Release approved command",
    ]
    .iter()
    .any(|trigger| prompt.contains(trigger));
  let escalation = prompt
    .split_once("Escalate `")
    .and_then(|(_, rest)| rest.split_once('`'))
    .map(|(command, _)| command.to_owned());
  let patch = prompt.contains("Patch outside");
  let design = prompt.contains("Use the design tools");
  let unasked = prompt.contains("Call a tool unasked");
  let twice = prompt.contains("Design twice");
  let question = prompt.contains(fake_codex_question::TRIGGER);
  if !fixture && escalation.is_none() && !patch && !design && !unasked && !twice && !question {
    return None;
  }
  session.approvals = Some(Approvals {
    prompt: prompt.to_owned(),
    structured,
    hold_approved: prompt.contains("Hold approved command"),
    hold_mcp: prompt.contains("Hold approved MCP"),
    release_command: prompt.contains("Release approved command"),
  });
  if let Some(command) = escalation {
    notify(
      session,
      "item/started",
      json!({"item": {"id": "escalation-1", "type": "commandExecution", "command": command}}),
    );
    let cwd = session.cwd.to_string_lossy().into_owned();
    request(
      session,
      ESCALATION,
      "item/commandExecution/requestApproval",
      json!({"itemId": "escalation-1", "startedAtMs": 1, "kind": "command", "environmentId": "local",
        "command": command, "cwd": cwd, "reason": "Do you want to reach the package registry?",
        "availableDecisions": ["accept", "acceptForSession", "decline"]}),
    );
  } else if question {
    fake_codex_question::ask(session);
  } else if design {
    design_calls(session);
  } else if unasked || twice {
    tool_request(session, "pencil", "batch_design");
  } else if patch {
    request(
      session,
      PATCH,
      "item/fileChange/requestApproval",
      json!({"itemId": "patch-1", "startedAtMs": 1, "reason": "Write outside the writable roots", "grantRoot": "/elsewhere"}),
    );
  } else {
    command_request(session);
  }
  Some(Step::Go)
}

/// Takes `answer` to one of the requests.
pub fn answered(session: &mut Session, answer: &Value) -> Step {
  let Some((prompt, structured)) = session
    .approvals
    .as_ref()
    .map(|approvals| (approvals.prompt.clone(), approvals.structured))
  else {
    eprintln!("an answer to no request: {answer}");
    return Step::Exit(ExitCode::from(2));
  };
  let decision = answer["result"]["decision"].as_str().unwrap_or_default().to_owned();
  match answer["id"].as_u64() {
    Some(COMMAND) => command_answered(session, answer),
    Some(MCP) if prompt.contains("Design twice") => twice_answered(session, answer, "mcp-1"),
    Some(SECOND_MCP) => twice_answered(session, answer, "mcp-2"),
    Some(MCP) => mcp_answered(session, answer),
    Some(ESCALATION) => {
      eprintln!("escalation: {decision}");
      if decision != "accept" {
        notify(
          session,
          "item/completed",
          json!({"item": {"id": "escalation-1", "type": "commandExecution", "command": "escalated", "status": "declined"}}),
        );
        return went_on(session, &prompt, structured);
      }
      if append(&session.cwd, "escalated").is_err() {
        eprintln!("cannot write README.md");
      }
      let report = report_of(&prompt, structured);
      notify(
        session,
        "item/completed",
        json!({"item": {"id": "escalation-1", "type": "commandExecution", "command": "escalated", "status": "completed", "exitCode": 0, "aggregatedOutput": ""}}),
      );
      finish(session, "hello", &report)
    }
    Some(PATCH) => {
      eprintln!("file change: {decision}");
      finish(session, "hello", &report_of(&prompt, structured))
    }
    Some(fake_codex_question::QUESTION) => fake_codex_question::answered(session, answer, (&prompt, structured)),
    _ => {
      eprintln!("an answer to an unknown request: {answer}");
      Step::Exit(ExitCode::from(2))
    }
  }
}

fn command_answered(session: &mut Session, answer: &Value) -> Step {
  if answer["result"] == json!({"decision": "decline"}) {
    notify(
      session,
      "item/completed",
      json!({"item": {"id": "command-1", "type": "commandExecution", "command": "disposable-command",
        "status": "declined"}}),
    );
    let Some((prompt, structured)) = session
      .approvals
      .as_ref()
      .map(|approvals| (approvals.prompt.clone(), approvals.structured))
    else {
      return Step::Exit(ExitCode::from(2));
    };
    return went_on(session, &prompt, structured);
  }
  if answer["result"] != json!({"decision": "accept"}) {
    eprintln!("only an exact action grant: {answer}");
    return Step::Exit(ExitCode::from(2));
  }
  let Some(approvals) = session.approvals.as_ref() else {
    return Step::Exit(ExitCode::from(2));
  };
  if fs::write(session.cwd.join("command-once.txt"), "approved").is_err() {
    eprintln!("cannot write command-once.txt");
  }
  if approvals.hold_approved {
    return Step::Go;
  }
  if approvals.release_command {
    while !session.cwd.join("release-active.txt").exists() {
      std::thread::sleep(std::time::Duration::from_millis(10));
    }
  }
  notify(
    session,
    "item/completed",
    json!({"item": {"id": "command-1", "type": "commandExecution", "command": "disposable-command",
      "status": "completed", "exitCode": 0, "aggregatedOutput": ""}}),
  );
  tool_request(session, "disposable", "write");
  Step::Go
}

fn mcp_answered(session: &mut Session, answer: &Value) -> Step {
  let unasked = session
    .approvals
    .as_ref()
    .is_some_and(|approvals| approvals.prompt.contains("Call a tool unasked"));
  if unasked {
    eprintln!(
      "mcp answer: {}",
      answer["result"]["action"].as_str().unwrap_or_default()
    );
  }
  if unasked && answer["result"]["action"] == "decline" {
    notify(
      session,
      "item/completed",
      json!({"item": {"id": "mcp-1", "type": "mcpToolCall", "server": "pencil", "tool": "batch_design",
        "status": "failed"}}),
    );
    let report = session
      .approvals
      .as_ref()
      .map(|approvals| report_of(&approvals.prompt, approvals.structured));
    return report.map_or(Step::Exit(ExitCode::from(2)), |report| {
      finish(session, "hello", &report)
    });
  }
  if answer["result"]["action"] == "decline" {
    notify(
      session,
      "item/completed",
      json!({"item": {"id": "mcp-1", "type": "mcpToolCall", "status": "failed"}}),
    );
    let Some((prompt, structured)) = session
      .approvals
      .as_ref()
      .map(|approvals| (approvals.prompt.clone(), approvals.structured))
    else {
      return Step::Exit(ExitCode::from(2));
    };
    return went_on(session, &prompt, structured);
  }
  if answer["result"]["action"] != "accept" {
    eprintln!("only an accepted elicitation continues: {answer}");
    return Step::Exit(ExitCode::from(2));
  }
  if fs::write(session.cwd.join("mcp-once.txt"), "approved").is_err() {
    eprintln!("cannot write mcp-once.txt");
  }
  if session.approvals.as_ref().is_some_and(|approvals| approvals.hold_mcp) {
    return Step::Go;
  }
  notify(
    session,
    "item/completed",
    json!({"item": {"id": "mcp-1", "type": "mcpToolCall", "status": "completed"}}),
  );
  message(
    session,
    "message-1",
    &json!({"outcome": "done", "summary": "continued"}).to_string(),
  );
  complete(session, None);
  Step::Exit(ExitCode::SUCCESS)
}

/// A declined request: the model was told and works on without it.
fn went_on(session: &mut Session, prompt: &str, structured: bool) -> Step {
  eprintln!("went on");
  if append(&session.cwd, "went on").is_err() {
    eprintln!("cannot write README.md");
  }
  finish(session, "hello", &report_of(prompt, structured))
}

/// An answer to one of the two calls of "Design twice": an approved call appends `approved`; the first
/// is followed by the second, the second by the end of the turn.
fn twice_answered(session: &mut Session, answer: &Value, item: &str) -> Step {
  let accepted = answer["result"]["action"] == "accept";
  eprintln!("{item}: {}", answer["result"]["action"].as_str().unwrap_or_default());
  let status = if accepted { "completed" } else { "failed" };
  if accepted && append(&session.cwd, "approved").is_err() {
    eprintln!("cannot write README.md");
  }
  notify(
    session,
    "item/completed",
    json!({"item": {"id": item, "type": "mcpToolCall", "server": "pencil", "tool": "batch_design", "status": status}}),
  );
  if !accepted && append(&session.cwd, "went on").is_err() {
    eprintln!("cannot write README.md");
  }
  if item == "mcp-1" {
    notify(
      session,
      "item/started",
      json!({"item": {"id": "mcp-2", "type": "mcpToolCall", "server": "pencil", "tool": "batch_design",
        "arguments": {}}}),
    );
    request(
      session,
      SECOND_MCP,
      "mcpServer/elicitation/request",
      json!({"serverName": "pencil", "mode": "form", "requestedSchema": {"type": "object", "properties": {}}}),
    );
    return Step::Go;
  }
  let Some((prompt, structured)) = session
    .approvals
    .as_ref()
    .map(|approvals| (approvals.prompt.clone(), approvals.structured))
  else {
    return Step::Exit(ExitCode::from(2));
  };
  finish(session, "hello", &report_of(&prompt, structured))
}

fn command_request(session: &Session) {
  notify(
    session,
    "item/started",
    json!({"item": {"id": "command-1", "type": "commandExecution", "command": "disposable-command"}}),
  );
  let cwd = session.cwd.to_string_lossy().into_owned();
  request(
    session,
    COMMAND,
    "item/commandExecution/requestApproval",
    json!({"itemId": "command-1", "command": "disposable-command", "cwd": cwd,
      "availableDecisions": ["accept", "decline"]}),
  );
}

/// Calls `tool` of `server` and asks about it; approved, it writes `mcp-once.txt`.
fn tool_request(session: &Session, server: &str, tool: &str) {
  notify(
    session,
    "item/started",
    json!({"item": {"id": "mcp-1", "type": "mcpToolCall", "server": server, "tool": tool,
      "arguments": {"filePath": "mcp-once.txt"}}}),
  );
  request(
    session,
    MCP,
    "mcpServer/elicitation/request",
    json!({"serverName": server, "mode": "form", "requestedSchema": {"type": "object", "properties": {}}}),
  );
}

/// Reads the design, unasked only when approved by configuration, then changes it; `pencil` by the name Codex got it
/// by (`fake_codex_user::argument_name`).
fn design_calls(session: &Session) {
  let pencil = argument_name(&session.args, "pencil");
  let read = json!({"id": "mcp-read", "type": "mcpToolCall", "server": pencil, "tool": "get_screenshot",
    "arguments": {}});
  if !session.approved_tools.contains(&format!("{pencil}/get_screenshot")) {
    notify(session, "item/started", json!({"item": read}));
    request(
      session,
      MCP,
      "mcpServer/elicitation/request",
      json!({"serverName": pencil, "mode": "form", "requestedSchema": {"type": "object", "properties": {}}}),
    );
    return;
  }
  notify(session, "item/started", json!({"item": read}));
  eprintln!("unasked: {pencil}/get_screenshot");
  if fs::write(session.cwd.join("read-once.txt"), "read").is_err() {
    eprintln!("cannot write read-once.txt");
  }
  let mut done = read;
  done["status"] = json!("completed");
  notify(session, "item/completed", json!({"item": done}));
  tool_request(session, &pencil, "batch_design");
}
