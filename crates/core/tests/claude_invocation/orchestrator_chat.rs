//! The Orchestrator chat's turn (`ClaudeChat`): read-only and only inside the project folder, with its own MCP
//! server's tools allowed and the folders that hold secrets denied by rule; and what its system prompt tells it about
//! links.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::invocation::{ClaudeChat, ClaudeSession},
  id::ProjectId,
  orchestrator::{
    MAX_TURNS,
    instructions::{ProjectReads, system_prompt},
  },
};

use super::SESSION;

fn chat_args(denied: &[PathBuf]) -> Vec<String> {
  chat_args_with(denied, &[])
}

fn chat_args_with(denied: &[PathBuf], denied_files: &[PathBuf]) -> Vec<String> {
  ClaudeChat {
    executable: Path::new("C:/tools/claude.exe"),
    model: "haiku",
    effort: None,
    folder: Path::new("C:/work/kontur"),
    session: ClaudeSession::New(SESSION),
    system_prompt: Path::new("C:/tmp/prompt.md"),
    mcp_config: Path::new("C:/tmp/mcp.json"),
    denied,
    denied_files,
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
  // The private folder as Windows may name it, short and long, and the Default account's own file.
  let denied = [
    PathBuf::from(r"C:\Users\ME~1\AppData\Local\Temp\orchester-mcp"),
    PathBuf::from(r"\\?\C:\Users\me\AppData\Local\Temp\orchester-mcp"),
    PathBuf::from(r"C:\Users\me\.claude"),
  ];
  let args = chat_args_with(&denied, &[PathBuf::from(r"C:\Users\me\.claude.json")]);
  assert_eq!(
    after(&args, "--settings"),
    [r#"{"permissions":{"blockReadsOutsideWorkingDirectories":true}}"#],
    "the CLI's own block on reads outside the working folder"
  );
  assert_eq!(after(&args, "--tools"), ["Read,Glob,Grep"]);
  // A bare `Read` would allow every path the person can read; without a rule a read outside the working folder
  // asks, which print mode denies.
  assert_eq!(after(&args, "--allowedTools"), ["mcp__orchester"]);
  assert_eq!(
    after(&args, "--disallowedTools"),
    [
      "Read(//c/Users/ME~1/AppData/Local/Temp/orchester-mcp/**)",
      "Read(//c/Users/me/AppData/Local/Temp/orchester-mcp/**)",
      "Read(//c/Users/me/.claude/**)",
      "Read(//c/Users/me/.claude.json)"
    ]
  );
  assert_eq!(after(&args, "--permission-mode"), ["default"]);
  assert_eq!(
    after(&args, "--max-turns"),
    [MAX_TURNS.to_string().as_str()],
    "one answer cannot run away"
  );
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

#[test]
fn orchestrator_links_only_with_the_links_its_tools_returned() {
  let prompt = system_prompt("Kontur", ProjectId::generate(), ProjectReads::FileTools);
  assert!(
    prompt.contains("[KON-12](orchester:/project/<id>/tasks/KON-12)"),
    "it shows how a `link` becomes an `orchester:` link: {prompt}"
  );
  assert!(
    prompt.contains("Never write or change a route yourself"),
    "it never makes up a route: {prompt}"
  );
  assert!(
    prompt.contains("only with the `url` a tool returned"),
    "it links a web page only with a returned `url`: {prompt}"
  );
  assert!(
    prompt.contains(
      "A link is always the Markdown form [words](orchester:<link>): never put a link, a route or a key you link \
       in backticks or a code block, and never show a route bare."
    ),
    "it never writes a link as code or a route bare: {prompt}"
  );
  assert!(
    prompt.contains("`- Label: [words](orchester:<link>) — detail`"),
    "it writes how several things stand as labelled links: {prompt}"
  );
}
