//! The fake Codex's answers to the read-only commands a connection check runs
//! (`orchester_core::connection::check`): its version, that it is signed in, and a catalog of one listed
//! model, so a connection to the fake can be added and checked the way a person adds Codex.

use std::process::ExitCode;

/// The model the fake's catalog lists, with the efforts it takes.
const CATALOG: &str = r#"{"models":[{"slug":"gpt-5.5","visibility":"list","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"}]}]}"#;

/// Answers `args` when they are a check's command; `None` for anything else.
pub fn answer(args: &[String]) -> Option<ExitCode> {
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let output = match args.as_slice() {
    ["--version"] => "codex-cli 0.139.0",
    ["login", "status"] => "Logged in using ChatGPT",
    ["debug", "models", "--bundled"] => CATALOG,
    _ => return None,
  };
  println!("{output}");
  Some(ExitCode::SUCCESS)
}
