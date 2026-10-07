//! `orchester-fake-claude` as auto mode's reviewer: started in print mode with a system prompt file, it
//! reads the review prompt as plain text from standard input and answers with Claude Code's
//! `--output-format stream-json` events: its `init`, then the result with the verdict in `structured_output`
//! and the review's usage ([`USAGE`]). What it answers depends on the task in the prompt: "Reviewer asks"
//! asks the person, "Reviewer babbles" prints prose instead of JSON, "Reviewer waits" approves after three
//! seconds, "Reviewer hangs" never answers, anything else approves; "Reviewer reports limits" adds a
//! `rate_limit_event` ([`LIMITS`]), "Reviewer hides usage" leaves the usage out of the result, and "Reviewer
//! rambles" prints more than the runner keeps of its output (320 events of 1 KiB) before its result. It
//! reports the arguments it got on standard error (`review argv: `). When the variable
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
  args.iter().any(|arg| arg == "--system-prompt-file")
}

pub fn review(args: &[String]) -> ExitCode {
  eprintln!("review argv: {}", args.join(" "));
  log_place(args);
  let mut prompt = String::new();
  if let Err(error) = std::io::stdin().read_to_string(&mut prompt) {
    eprintln!("cannot read the review prompt: {error}");
    return ExitCode::from(2);
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

/// Appends where the reviewer runs to the file `FAKE_REVIEW_LOG` names, if any.
fn log_place(args: &[String]) {
  let Some(log) = std::env::var_os("FAKE_REVIEW_LOG") else {
    return;
  };
  let cwd = std::env::current_dir().expect("a working folder");
  let empty = std::fs::read_dir(&cwd).is_ok_and(|mut entries| entries.next().is_none());
  let instructions = args
    .windows(2)
    .find(|pair| pair[0] == "--system-prompt-file")
    .and_then(|pair| std::fs::read_to_string(&pair[1]).ok())
    .is_some_and(|text| text.contains("untrusted data"));
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(log)
    .expect("open the review log");
  writeln!(
    file,
    "pid {}\ncwd {}\nempty {empty}\ninstructions {instructions}",
    std::process::id(),
    cwd.display()
  )
  .expect("write the review log");
}
