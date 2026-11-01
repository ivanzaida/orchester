//! `orchester-fake-claude` as auto mode's reviewer: started in print mode with a system prompt file, it
//! reads the review prompt as plain text from standard input and answers with Claude Code's
//! `--output-format stream-json` events: its `init`, then the result with the verdict in `structured_output`
//! and the review's usage ([`USAGE`]). What it answers depends on the task in the prompt: "Reviewer asks"
//! asks the person, "Reviewer babbles" prints prose instead of JSON, "Reviewer signed out" fails as a signed-out
//! Claude Code does ([`signed_out`]), "Reviewer waits" approves after three
//! seconds, "Reviewer hangs" never answers, anything else approves; "Reviewer reports limits" adds a
//! `rate_limit_event` ([`LIMITS`]), "Reviewer hides usage" leaves the usage out of the result, and "Reviewer
//! rambles" prints more than the runner keeps of its output (320 events of 1 KiB) before its result. It
//! reports the arguments it got on standard error (`review argv: `). Asked for a task's story points (ORC-49), it
//! answers as the estimator ([`estimate`]). When the variable
//! `FAKE_REVIEW_LOG` names a file, it appends where it ran to it: `pid <id>`, `cwd <folder>`, `empty
//! <whether the folder was empty>` and `instructions <whether its system prompt file is the reviewer's>`.

use std::{
  io::{Read, Write},
  process::ExitCode,
  time::Duration,
};

/// What every review reports it spent. Counted: 120 + 30 (cache writes) + 7 = 157; the 50 cache reads
/// do not count.
pub const USAGE: &str =
  r#"{"input_tokens":120,"output_tokens":7,"cache_creation_input_tokens":30,"cache_read_input_tokens":50}"#;
/// The limits a review reports when asked to: the weekly window 42% used.
pub const LIMITS: &str =
  r#"{"status":"allowed","resetsAt":1790812200,"rateLimitType":"seven_day","utilization":0.42,"isUsingOverage":false}"#;

/// Whether `args` start the reviewer rather than an agent: only the reviewer gets its system prompt as a file.
pub fn is_review(args: &[String]) -> bool {
  args.iter().any(|arg| arg == "--system-prompt-file") && !args.iter().any(|arg| arg == "--mcp-config")
}

pub fn review(args: &[String]) -> ExitCode {
  eprintln!("review argv: {}", args.join(" "));
  log_place(args);
  let mut prompt = String::new();
  if let Err(error) = std::io::stdin().read_to_string(&mut prompt) {
    eprintln!("cannot read the review prompt: {error}");
    return ExitCode::from(2);
  }
  if prompt.contains("Give the task's story points.") {
    return estimate(&prompt);
  }
  log_line(&format!(
    "roots {}",
    prompt.contains("does the request touch anything outside the roots")
  ));
  if prompt.contains("Reviewer signed out") {
    return signed_out(&prompt);
  }
  if prompt.contains("Reviewer babbles") {
    println!("Sure, that looks fine to me.");
    return ExitCode::SUCCESS;
  }
  if prompt.contains("Reviewer hangs") {
    std::thread::sleep(Duration::from_secs(120));
  }
  if prompt.contains("Reviewer waits") {
    std::thread::sleep(Duration::from_secs(3));
  }
  let (verdict, reason) = if prompt.contains("Reviewer asks") {
    ("ask", "The fake reviewer leaves it to the person.")
  } else {
    ("approve", "The fake reviewer finds it serves the task.")
  };
  println!(
    "{}",
    serde_json::json!({"type": "system", "subtype": "init", "session_id": "fake-review"})
  );
  if prompt.contains("Reviewer rambles") {
    let thinking = "x".repeat(1024);
    for _ in 0..320 {
      println!(
        "{}",
        serde_json::json!({"type": "assistant", "message": {"content": [{"type": "thinking", "thinking": thinking}]}})
      );
    }
  }
  if prompt.contains("Reviewer reports limits") {
    println!(r#"{{"type":"rate_limit_event","rate_limit_info":{LIMITS}}}"#);
  }
  let mut answer = serde_json::json!({
    "type": "result",
    "subtype": "success",
    "is_error": false,
    "result": "",
    "structured_output": {"verdict": verdict, "reason": reason},
  });
  if !prompt.contains("Reviewer hides usage") {
    answer["usage"] = serde_json::from_str(USAGE).expect("the usage is JSON");
  }
  println!("{answer}");
  ExitCode::SUCCESS
}

/// Claude Code signed out, as it answers in print mode
/// (`crates/core/tests/fixtures/claude/check-now-signed-out.jsonl`): an error result and exit code 1. Its error also
/// carries the value of `FAKE_REVIEW_SECRET` and the whole prompt, and its error output the value again, which must not
/// reach the review or the log.
fn signed_out(prompt: &str) -> ExitCode {
  let secret = std::env::var("FAKE_REVIEW_SECRET").unwrap_or_default();
  eprintln!("token {secret}");
  let said = format!("Not logged in · Please run /login\nkey {secret}\n{prompt}");
  println!(
    "{}",
    serde_json::json!({"type": "result", "subtype": "success", "is_error": true, "result": said})
  );
  ExitCode::from(1)
}

/// The story-point estimator's answer on the same connection: "Estimator babbles" prints prose; a task whose brief
/// says "Large" gets 8 points, any other 3, with the review's usage. The reason names how many merged tasks the
/// prompt gave as examples (`examples <n>`), counted by their keys.
fn estimate(prompt: &str) -> ExitCode {
  if prompt.contains("Estimator babbles") {
    println!("About a three, I'd say.");
    return ExitCode::SUCCESS;
  }
  let points = if prompt.contains("Large") { 8 } else { 3 };
  let examples = prompt.matches("\"key\":").count();
  println!(
    "{}",
    serde_json::json!({"type": "system", "subtype": "init", "session_id": "fake-estimate"})
  );
  let answer = serde_json::json!({
    "type": "result",
    "subtype": "success",
    "is_error": false,
    "result": "",
    "structured_output": {"points": points, "reason": format!("Sized by the fake estimator; examples {examples}.")},
    "usage": serde_json::from_str::<serde_json::Value>(USAGE).expect("the usage is JSON"),
  });
  println!("{answer}");
  ExitCode::SUCCESS
}

/// Appends where the reviewer runs to the file `FAKE_REVIEW_LOG` names, if any.
fn log_place(args: &[String]) {
  let cwd = std::env::current_dir().expect("a working folder");
  let empty = std::fs::read_dir(&cwd).is_ok_and(|mut entries| entries.next().is_none());
  let instructions = args
    .windows(2)
    .find(|pair| pair[0] == "--system-prompt-file")
    .and_then(|pair| std::fs::read_to_string(&pair[1]).ok())
    .is_some_and(|text| text.contains("untrusted data"));
  log_line(&format!(
    "pid {}\ncwd {}\nempty {empty}\ninstructions {instructions}",
    std::process::id(),
    cwd.display()
  ));
}

/// Appends `line` to the file `FAKE_REVIEW_LOG` names, if any.
fn log_line(line: &str) {
  let Some(log) = std::env::var_os("FAKE_REVIEW_LOG") else {
    return;
  };
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(log)
    .expect("open the review log");
  writeln!(file, "{line}").expect("write the review log");
}
