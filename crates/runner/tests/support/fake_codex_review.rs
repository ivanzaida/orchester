//! `orchester-fake-codex` as auto mode's reviewer: started as `codex -a never exec … -`, it reads the
//! instructions and the review message from standard input and answers with `codex exec --json`'s events,
//! the verdict's JSON as the text of the final `agent_message`, as Codex does with `--output-schema`. What
//! it answers depends on the message: "Reviewer asks" asks the person, "Reviewer babbles" ends with prose,
//! "Reviewer fails" reports a failed turn, anything else approves. When the variable `FAKE_REVIEW_LOG`
//! names a file, it appends how it ran to it: `argv <arguments>`, `pid <id>`, `cwd <folder>`, `empty
//! <whether the folder was empty>`, `schema <whether its --output-schema file is the verdict's schema>`
//! and `instructions <whether the message starts with the reviewer's instructions>`.

use std::{
  io::{Read, Write},
  process::ExitCode,
};

use serde_json::{Value, json};

/// Whether `args` start the reviewer rather than App Server.
pub fn is_review(args: &[String]) -> bool {
  args.iter().any(|arg| arg == "exec")
}

pub fn review(args: &[String]) -> ExitCode {
  let mut prompt = String::new();
  if let Err(error) = std::io::stdin().read_to_string(&mut prompt) {
    eprintln!("cannot read the review prompt: {error}");
    return ExitCode::from(2);
  }
  log_place(args, &prompt);
  let event = |value: Value| println!("{value}");
  event(json!({"type": "thread.started", "thread_id": "fake-review-thread"}));
  event(json!({"type": "turn.started"}));
  if prompt.contains("Reviewer fails") {
    event(json!({"type": "turn.failed", "error": {"message": "the fake model failed"}}));
    return ExitCode::from(1);
  }
  let text = if prompt.contains("Reviewer babbles") {
    "Looks fine to me.".to_owned()
  } else if prompt.contains("Reviewer asks") {
    json!({"verdict": "ask", "reason": "The fake Codex reviewer leaves it to the person."}).to_string()
  } else {
    json!({"verdict": "approve", "reason": "The fake Codex reviewer finds it serves the task."}).to_string()
  };
  event(json!({"type": "item.completed", "item": {"id": "item_0", "type": "agent_message", "text": text}}));
  event(json!({"type": "turn.completed", "usage": {"input_tokens": 1, "output_tokens": 1}}));
  ExitCode::SUCCESS
}

/// Appends how the reviewer runs to the file `FAKE_REVIEW_LOG` names, if any.
fn log_place(args: &[String], prompt: &str) {
  let Some(log) = std::env::var_os("FAKE_REVIEW_LOG") else {
    return;
  };
  let cwd = std::env::current_dir().expect("a working folder");
  let empty = std::fs::read_dir(&cwd).is_ok_and(|mut entries| entries.next().is_none());
  let schema = args
    .windows(2)
    .find(|pair| pair[0] == "--output-schema")
    .and_then(|pair| std::fs::read_to_string(&pair[1]).ok())
    .is_some_and(|text| text.contains("\"verdict\""));
  let instructions = prompt.starts_with("You are Orchester's request reviewer.");
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(log)
    .expect("open the review log");
  writeln!(
    file,
    "argv {}\npid {}\ncwd {}\nempty {empty}\nschema {schema}\ninstructions {instructions}",
    args.join(" "),
    std::process::id(),
    cwd.display()
  )
  .expect("write the review log");
}
