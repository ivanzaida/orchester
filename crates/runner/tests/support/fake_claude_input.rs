//! `orchester-fake-claude`'s stream-json input (`--input-format stream-json --replay-user-messages`),
//! as Claude Code 2.1.286 handled it in the probe of 2026-10-02: the first user message is the prompt;
//! a message that arrives during a tool call is taken right after that call's result, and one that
//! arrives after the last tool call is taken as the next turn, after the result. Each is printed back
//! (`isReplay`) as it is taken. The CLI exits once its input ends and it has taken what it was given.
//! Each message is reported on standard error as it arrives (`input: <message>`). A prompt that says
//! "Ignore messages" makes it read nothing after the prompt, as a CLI that stopped reading its input.

use std::{
  io::BufRead,
  sync::{Mutex, OnceLock, mpsc},
  time::Duration,
};

use serde_json::{Value, json};

static INPUT: OnceLock<Mutex<mpsc::Receiver<String>>> = OnceLock::new();

/// Reads the prompt, the first user message, and keeps reading the rest of the input in the
/// background. `None` when the first line is not a user message. The kinds of the first message's content blocks are
/// reported on standard error (`input-blocks: text,text,image:image/png`), and kept for the chat's log ([`blocks`]).
pub fn open() -> Option<String> {
  let mut lines = std::io::stdin().lock().lines();
  let first = lines.next()?.ok()?;
  let prompt = text_of(&first)?;
  let kinds = blocks_of(&first);
  eprintln!("input-blocks: {kinds}");
  BLOCKS.set(kinds).ok()?;
  drop(lines);
  if prompt.contains("Ignore messages") {
    return Some(prompt);
  }
  let (sender, receiver) = mpsc::channel();
  std::thread::spawn(move || {
    for line in std::io::stdin().lock().lines() {
      let Ok(line) = line else {
        return;
      };
      match text_of(&line) {
        Some(text) if sender.send(text.clone()).is_ok() => {
          eprintln!("input: {text}");
          eprintln!("input-blocks: {}", blocks_of(&line));
        }
        Some(_) => return,
        None => eprintln!("not a user message: {line}"),
      }
    }
  });
  INPUT.set(Mutex::new(receiver)).ok()?;
  Some(prompt)
}

static BLOCKS: OnceLock<String> = OnceLock::new();

/// The kinds of the first message's content blocks, as [`open`] reported them.
pub fn blocks() -> String {
  BLOCKS.get().cloned().unwrap_or_default()
}

fn text_of(line: &str) -> Option<String> {
  let value: Value = serde_json::from_str(line).ok()?;
  if value["type"] != "user" {
    return None;
  }
  value["message"]["content"][0]["text"].as_str().map(str::to_owned)
}

/// Each content block of the user message `line` by its type, an image or a document with its media type.
fn blocks_of(line: &str) -> String {
  let value: Value = serde_json::from_str(line).unwrap_or_default();
  value["message"]["content"]
    .as_array()
    .into_iter()
    .flatten()
    .map(
      |block| match (block["type"].as_str(), block["source"]["media_type"].as_str()) {
        (Some(kind), Some(media)) => format!("{kind}:{media}"),
        (kind, _) => kind.unwrap_or("?").to_owned(),
      },
    )
    .collect::<Vec<_>>()
    .join(",")
}

/// The next message that arrived, if any, waiting at most `wait`.
pub fn next(wait: Duration) -> Option<String> {
  let input = INPUT.get()?.lock().ok()?;
  input.recv_timeout(wait).ok()
}

/// Prints `text` back as the CLI does when it takes a user message.
pub fn replay(session: &str, text: &str) {
  println!(
    "{}",
    json!({"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": text}]},
      "session_id": session, "isReplay": true})
  );
}

/// After a result: every message still to come is a turn of its own, until the input ends.
pub fn later_turns(session: &str) {
  let Some(input) = INPUT.get().and_then(|input| input.lock().ok()) else {
    return;
  };
  while let Ok(text) = input.recv() {
    println!(
      r#"{{"type":"system","subtype":"init","cwd":"worktree","session_id":"{session}","tools":[],"mcp_servers":[],"model":"claude-haiku-4-5","permissionMode":"acceptEdits","apiKeySource":"none","claude_code_version":"2.1.286"}}"#
    );
    replay(session, &text);
    let result = json!({"outcome": "done", "summary": format!("Read after the turn: {text}")}).to_string();
    println!(
      "{}",
      json!({"type": "assistant", "session_id": session, "message": {"id": format!("msg_later_{}", text.len()),
        "role": "assistant", "type": "message", "content": [{"type": "text", "text": result}],
        "usage": {"input_tokens": 0, "output_tokens": 0}}})
    );
    println!(
      "{}",
      json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1, "result": result,
        "session_id": session, "usage": {"input_tokens": 0, "output_tokens": 0}, "permission_denials": []})
    );
    flush();
  }
}

fn flush() {
  if std::io::Write::flush(&mut std::io::stdout()).is_err() {
    eprintln!("cannot flush standard output");
  }
}
