//! "Search the code index" (ORC-90): `orchester-fake-codex` searches the code index its prompt names as Codex 0.159.2
//! reports such work over App Server: a call of Orchester's `find_symbol` (made through the thread's relay and
//! reported as an `mcpToolCall` item), an `rg` command in the index folder as the internal skill shows it, and a
//! command a sub-agent's thread runs through `$IDX` (Orchester turns Codex's sub-agents off for a step; should one run,
//! its thread's items come over the same connection); then an `rg` over the worktree, which is no index search. It
//! reports each (`index-command: <id>`, `index-call: find_symbol <status>`).

use std::path::Path;

use serde_json::json;

use super::{Session, emit, notify};

#[path = "fake_index_prompt.rs"]
mod fake_index_prompt;

/// A thread of a sub-agent of the session's thread.
const SUB_AGENT_THREAD: &str = "fake-thread-sub-agent";

pub fn search(session: &mut Session, prompt: &str) {
  let Some(folder) = fake_index_prompt::index_folder(prompt) else {
    eprintln!("index-call: the prompt names no code index");
    return;
  };
  let arguments = json!({"name": "parse_config"});
  let answer = session
    .orchester
    .as_mut()
    .and_then(|server| server.call("find_symbol", &arguments));
  let status = if answer.as_ref().is_some_and(|(_, failed)| !failed) {
    "completed"
  } else {
    "failed"
  };
  eprintln!("index-call: find_symbol {status}");
  let call = json!({"id": "index-find", "type": "mcpToolCall", "server": "Orchester", "tool": "find_symbol",
    "arguments": arguments});
  notify(session, "item/started", json!({"item": call}));
  let mut done = call;
  done["status"] = json!(status);
  notify(session, "item/completed", json!({"item": done}));
  let thread = session.thread.clone();
  let symbols = Path::new(&folder).join("symbols.tsv");
  command(
    session,
    ("index-rg", &thread),
    &format!("rg -n '^parse_config\\t' '{}'", symbols.display()),
  );
  let assigned = format!("$IDX = '{folder}'; rg -n '^[^\\t]*config' \"$IDX/tree.txt\"");
  command(session, ("index-sub-agent", SUB_AGENT_THREAD), &assigned);
  command(session, ("worktree-rg", &thread), "rg -n parse_config src");
}

/// A command `id` of `thread` that ran and found something, reported as Codex reports it.
fn command(session: &Session, (id, thread): (&str, &str), command: &str) {
  eprintln!("index-command: {id}");
  let item = json!({"id": id, "type": "commandExecution", "command": command});
  let mut done = item.clone();
  done["status"] = json!("completed");
  done["exitCode"] = json!(0);
  done["aggregatedOutput"] = json!("parse_config\tsrc/lib.rs\t1\tfunction\t");
  for (method, item) in [("item/started", item), ("item/completed", done)] {
    emit(&json!({"method": method, "params": {"threadId": thread, "turnId": session.turn, "item": item}}));
  }
}
