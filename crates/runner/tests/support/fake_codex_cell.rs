//! A yielded code-mode cell: its MCP request outlives turn/completed and is collected in a later turn.
use std::{path::Path, process::ExitCode, sync::mpsc, thread, time::Duration};

use serde_json::json;

use super::{
  Session, Step, complete,
  fake_codex_work::{finish, message, report_of},
  fake_permission_prompt::Prompt,
  notify,
};

pub const TRIGGER: &str = "Yield outside read";
pub const CONTINUE: &str = "Collect the results of the Orchester tool calls";

pub struct Cell {
  answer: mpsc::Receiver<(Prompt, Option<(String, bool)>)>,
  prompt: String,
  structured: bool,
}

pub fn start(session: &mut Session, prompt: &str, structured: bool) -> Step {
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
  thread::spawn(move || {
    let result = server.call("search_external_file", &arguments);
    eprintln!("cell-returned: yes");
    // A terminated fake cannot collect the result.
    sender.send((server, result)).ok();
  });
  session.cell = Some(Cell {
    answer,
    prompt: prompt.to_owned(),
    structured,
  });
  wait_to_end(&session.cwd);
  message(
    session,
    "pending-cell",
    &json!({"outcome": "question", "summary": "Lookup pending in tool cell 14"}).to_string(),
  );
  complete(session, None);
  Step::Go
}

fn wait_to_end(worktree: &Path) {
  while !worktree.join("yield-turn.txt").exists() {
    thread::sleep(Duration::from_millis(10));
  }
}

pub fn collect(session: &mut Session) -> Step {
  let Some(cell) = session.cell.take() else {
    return Step::Exit(ExitCode::from(2));
  };
  let Ok((server, Some((text, failed)))) = cell.answer.recv_timeout(Duration::from_secs(15)) else {
    eprintln!("cell: no answer");
    return Step::Exit(ExitCode::from(2));
  };
  session.orchester = Some(server);
  eprintln!("cell: {}: {text}", if failed { "denied" } else { "approved" });
  notify(
    session,
    "item/completed",
    json!({"item": {"id": "cell-search", "type": "mcpToolCall",
    "server": "Orchester", "tool": "search_external_file", "status": "completed"}}),
  );
  finish(session, "collected", &report_of(&cell.prompt, cell.structured))
}
