//! A yielded code-mode cell: its MCP request outlives turn/completed and is collected in a later turn.
//!
//! - "Yield outside read": the cell calls Orchester's `search_external_file` outside the worktree; the model ends its
//!   turn (`pending-cell`) once `yield-turn.txt` lies in the worktree, and a continuation (`CONTINUE`) collects the
//!   cell's result (`cell: approved|denied: <text>`). Codex completes the MCP item when the call returns, unless the
//!   prompt also says "Never complete the call", as for a cell Codex abandoned. "Fail the turn" ends the turn failed
//!   instead, as a usage limit does. "Ask a command after" has the cell, once its call returned, ask about a command
//!   (`cell-command`) for the turn that started it; the continuation finishes once that is answered (`cell command:
//!   <decision>`). A continuation that comes before the answer waits for it, unless the prompt says "Give up waiting"
//!   (it ends its turn with the cell uncollected, as when Codex's wait times out) or "Ask another decision" (it asks
//!   about a second command, `another-command`, while the first waits).
//! - "Abandon an Orchester call": starts a call of Orchester's tools that never reaches the relay nor completes, then
//!   finishes the turn as usual. "Call a tool every turn" also calls `get_task` in that turn and in each continuation,
//!   which finishes again with the same result.
use std::{path::Path, process::ExitCode, sync::mpsc, thread, time::Duration};

use serde_json::{Value, json};

use super::{
  Session, Step, complete, emit,
  fake_codex_work::{StepReport, finish, message, report_of},
  fake_permission_prompt::Prompt,
  notify, request,
};

const TRIGGER: &str = "Yield outside read";
const CONTINUE: &str = "Collect the results of the Orchester tool calls";
const ABANDON: &str = "Abandon an Orchester call";
/// The id of the request the cell asks about its command.
pub const COMMAND: u64 = 500;
/// The id of the second command a continuation asks about while the first waits.
const ANOTHER: u64 = 501;

pub struct Cell {
  answer: mpsc::Receiver<(Prompt, Option<(String, bool)>)>,
  prompt: String,
  structured: bool,
  asks_command: bool,
}

/// The command a cell asked about: not yet, answered before the continuation collected the cell, or collected and
/// waiting for the answer to finish with this report.
#[derive(Default)]
pub enum Command {
  #[default]
  None,
  Answered(String),
  Collected(StepReport),
}

/// What a turn of `prompt` does with a cell, if it is about one.
pub fn work(session: &mut Session, prompt: &str, structured: bool) -> Option<Step> {
  if prompt.starts_with(CONTINUE) {
    Some(collect(session))
  } else if prompt.contains(TRIGGER) {
    Some(start(session, prompt, structured))
  } else if prompt.contains(ABANDON) {
    Some(abandon(session, prompt, structured))
  } else {
    None
  }
}

fn start(session: &mut Session, prompt: &str, structured: bool) -> Step {
  let Some(mut server) = session.orchester.take() else {
    return Step::Exit(ExitCode::from(2));
  };
  let folder = session.cwd.ancestors().nth(3).expect("repository").join("deps-a");
  let arguments = json!({"base_directory": folder, "term": "outside line"});
  notify(
    session,
    "item/started",
    json!({"item": {"id": "cell-search", "type": "mcpToolCall", "server": "Orchester",
      "tool": "search_external_file", "arguments": arguments}}),
  );
  let (sender, answer) = mpsc::channel();
  let (thread, turn) = (session.thread.clone(), session.turn.clone());
  let cwd = session.cwd.to_string_lossy().into_owned();
  let completes = !prompt.contains("Never complete the call");
  let asks_command = prompt.contains("Ask a command after");
  thread::spawn(move || {
    let result = server.call("search_external_file", &arguments);
    // Codex completes the MCP item when it returns, before the model collects the yielded cell.
    if completes {
      emit(
        &json!({"method": "item/completed", "params": {"threadId": thread, "turnId": turn,
        "item": {"id": "cell-search", "type": "mcpToolCall", "server": "Orchester",
          "tool": "search_external_file", "status": "completed"}}}),
      );
    }
    eprintln!("cell-returned: yes");
    if asks_command {
      ask_command(&thread, &turn, &cwd);
    }
    // A terminated fake cannot collect the result.
    sender.send((server, result)).ok();
  });
  session.cell = Some(Cell {
    answer,
    prompt: prompt.to_owned(),
    structured,
    asks_command,
  });
  wait_to_end(&session.cwd);
  message(
    session,
    "pending-cell",
    &json!({"outcome": "question", "summary": "Lookup pending in tool cell 14"}).to_string(),
  );
  complete(
    session,
    prompt.contains("Fail the turn").then_some("usage limit reached"),
  );
  Step::Go
}

/// The cell, still at work after its turn `turn` ended, asks about a command of that turn.
fn ask_command(thread: &str, turn: &str, cwd: &str) {
  emit(
    &json!({"method": "item/started", "params": {"threadId": thread, "turnId": turn,
    "item": {"id": "cell-command", "type": "commandExecution", "command": "cell-command"}}}),
  );
  emit(
    &json!({"id": COMMAND, "method": "item/commandExecution/requestApproval", "params": {
    "threadId": thread, "turnId": turn, "itemId": "cell-command", "command": "cell-command", "cwd": cwd,
    "availableDecisions": ["accept", "decline"]}}),
  );
}

/// A call of Orchester's tools Codex starts and drops: it never reaches the relay and never completes.
fn abandon(session: &mut Session, prompt: &str, structured: bool) -> Step {
  notify(
    session,
    "item/started",
    json!({"item": {"id": "abandoned-call", "type": "mcpToolCall", "server": "Orchester",
      "tool": "read_file", "arguments": {"path": "README.md"}}}),
  );
  session.calls_every_turn = prompt.contains("Call a tool every turn");
  if session.calls_every_turn {
    call_a_tool(session);
  }
  finish(session, "hello", &report_of(prompt, structured))
}

/// Calls `get_task` through the relay, which answers it at once.
fn call_a_tool(session: &mut Session) {
  let answer = session
    .orchester
    .as_mut()
    .and_then(|server| server.call("get_task", &json!({})));
  eprintln!(
    "every-turn call: {}",
    if answer.is_some() { "answered" } else { "no answer" }
  );
}

fn wait_to_end(worktree: &Path) {
  while !worktree.join("yield-turn.txt").exists() {
    thread::sleep(Duration::from_millis(10));
  }
}

fn collect(session: &mut Session) -> Step {
  let Some(cell) = session.cell.take() else {
    if session.calls_every_turn {
      call_a_tool(session);
    }
    let text = if session.calls_every_turn {
      session.completed_result.clone()
    } else {
      session.completed_result.take()
    };
    let Some(text) = text else {
      return Step::Exit(ExitCode::from(2));
    };
    message(session, "already-collected", &text);
    complete(session, None);
    return Step::Go;
  };
  if cell.asks_command && !matches!(session.cell_command, Command::Answered(_)) {
    if cell.prompt.contains("Give up waiting") {
      eprintln!("cell: gave up waiting");
      session.cell = Some(cell);
      message(
        session,
        "still-pending",
        &json!({"outcome": "question", "summary": "The command in tool cell 14 still waits"}).to_string(),
      );
      complete(session, None);
      return Step::Go;
    }
    if cell.prompt.contains("Ask another decision") {
      session.cell = Some(cell);
      let cwd = session.cwd.to_string_lossy().into_owned();
      notify(
        session,
        "item/started",
        json!({"item": {"id": "another-command", "type": "commandExecution", "command": "another-command"}}),
      );
      request(
        session,
        ANOTHER,
        "item/commandExecution/requestApproval",
        json!({"itemId": "another-command", "command": "another-command", "cwd": cwd,
          "availableDecisions": ["accept", "decline"]}),
      );
      return Step::Go;
    }
  }
  let Ok((server, Some((text, failed)))) = cell.answer.recv_timeout(Duration::from_secs(15)) else {
    eprintln!("cell: no answer");
    return Step::Exit(ExitCode::from(2));
  };
  session.orchester = Some(server);
  eprintln!("cell: {}: {text}", if failed { "denied" } else { "approved" });
  let report = report_of(&cell.prompt, cell.structured);
  if !cell.asks_command {
    return finish(session, "collected", &report);
  }
  match std::mem::take(&mut session.cell_command) {
    Command::Answered(decision) => command_done(session, &decision, &report),
    Command::None | Command::Collected(_) => {
      session.cell_command = Command::Collected(report);
      Step::Go
    }
  }
}

/// The answer to the cell's command, before or after the continuation collected the cell.
pub fn command_answered(session: &mut Session, answer: &Value) -> Step {
  let decision = answer["result"]["decision"].as_str().unwrap_or_default().to_owned();
  match std::mem::take(&mut session.cell_command) {
    Command::Collected(report) => command_done(session, &decision, &report),
    Command::None | Command::Answered(_) => {
      session.cell_command = Command::Answered(decision);
      Step::Go
    }
  }
}

fn command_done(session: &mut Session, decision: &str, report: &StepReport) -> Step {
  eprintln!("cell command: {decision}");
  let status = if decision == "accept" { "completed" } else { "declined" };
  notify(
    session,
    "item/completed",
    json!({"item": {"id": "cell-command", "type": "commandExecution", "command": "cell-command",
      "status": status, "exitCode": 0, "aggregatedOutput": ""}}),
  );
  finish(session, "collected", report)
}
