//! `orchester-fake-claude` as auto mode's reviewer: started in print mode with `--output-format json`, it
//! reads the review prompt as plain text from standard input and answers with the `--output-format json`
//! result of Claude Code, the verdict in `structured_output`. What it answers depends on the task in the
//! prompt: "Reviewer asks" asks the person, "Reviewer babbles" prints prose instead of JSON, "Reviewer
//! waits" approves after three seconds, "Reviewer hangs" never answers, anything else approves. It
//! reports the arguments it got on standard error (`review argv: `). When the variable
//! `FAKE_REVIEW_LOG` names a file, it appends where it ran to it: `pid <id>`, `cwd <folder>`, `empty
//! <whether the folder was empty>` and `instructions <whether its system prompt file is the reviewer's>`.

use std::{
  io::{Read, Write},
  process::ExitCode,
  time::Duration,
};

/// Whether `args` start the reviewer rather than an agent.
pub fn is_review(args: &[String]) -> bool {
  args
    .windows(2)
    .any(|pair| pair[0] == "--output-format" && pair[1] == "json")
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
  let answer = serde_json::json!({
    "type": "result",
    "subtype": "success",
    "is_error": false,
    "result": "",
    "structured_output": {"verdict": verdict, "reason": reason},
  });
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
