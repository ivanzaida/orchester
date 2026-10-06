//! `orchester-fake-claude` as auto mode's reviewer: started in print mode with `--output-format json`, it
//! reads the review prompt as plain text from standard input and answers with the `--output-format json`
//! result of Claude Code, the verdict in `structured_output`. What it answers depends on the task in the
//! prompt: "Reviewer asks" asks the person, "Reviewer babbles" prints prose instead of JSON, anything else
//! approves. It reports the arguments it got on standard error (`review argv: `).

use std::{io::Read, process::ExitCode};

/// Whether `args` start the reviewer rather than an agent.
pub fn is_review(args: &[String]) -> bool {
  args
    .windows(2)
    .any(|pair| pair[0] == "--output-format" && pair[1] == "json")
}

pub fn review(args: &[String]) -> ExitCode {
  eprintln!("review argv: {}", args.join(" "));
  let mut prompt = String::new();
  if let Err(error) = std::io::stdin().read_to_string(&mut prompt) {
    eprintln!("cannot read the review prompt: {error}");
    return ExitCode::from(2);
  }
  if prompt.contains("Reviewer babbles") {
    println!("Sure, that looks fine to me.");
    return ExitCode::SUCCESS;
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
