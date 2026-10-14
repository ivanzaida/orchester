//! The Orchestrator chat's turn (`ClaudeChat`): read-only and only inside the project folder, with its own MCP
//! server's tools allowed and the folders that hold secrets denied by rule.

use std::path::{Path, PathBuf};

use orchester_core::connection::invocation::{ClaudeChat, ClaudeSession};

use super::SESSION;

fn chat_args(denied: &[PathBuf]) -> Vec<String> {
  ClaudeChat {
    executable: Path::new("C:/tools/claude.exe"),
    model: "haiku",
    effort: None,
    folder: Path::new("C:/work/kontur"),
    session: ClaudeSession::New(SESSION),
    system_prompt: Path::new("C:/tmp/prompt.md"),
    mcp_config: Path::new("C:/tmp/mcp.json"),
    denied,
  }
  .args()
}

/// The values after `flag`, up to the next flag.
fn after<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
  args
    .iter()
    .skip_while(|arg| *arg != flag)
    .skip(1)
    .take_while(|arg| !arg.starts_with("--"))
    .map(String::as_str)
    .collect()
}

#[test]
fn orchestrator_chat_reads_only_in_the_project_folder_and_never_in_secret_folders() {
  let denied = [
    PathBuf::from(r"C:\Users\me\AppData\Local\Temp\orchester-mcp"),
    PathBuf::from(r"C:\Users\me\.claude"),
  ];
  let args = chat_args(&denied);
  assert_eq!(after(&args, "--tools"), ["Read,Glob,Grep"]);
  // A bare `Read` would allow every path the person can read; without a rule a read outside the working folder
  // asks, which print mode denies.
  assert_eq!(after(&args, "--allowedTools"), ["mcp__orchester"]);
  assert_eq!(
    after(&args, "--disallowedTools"),
    [
      "Read(//c/Users/me/AppData/Local/Temp/orchester-mcp/**)",
      "Read(//c/Users/me/.claude/**)"
    ]
  );
  assert_eq!(after(&args, "--permission-mode"), ["default"]);
}

#[test]
fn orchestrator_chat_denies_unix_folders_and_skips_what_it_cannot_name() {
  let denied = [
    PathBuf::from("/Users/me/Library/Application Support/claude"),
    PathBuf::from(r"\\server\share\accounts"),
  ];
  let args = chat_args(&denied);
  assert_eq!(
    after(&args, "--disallowedTools"),
    ["Read(//Users/me/Library/Application Support/claude/**)"]
  );
  assert!(!chat_args(&[]).iter().any(|arg| arg == "--disallowedTools"));
}
