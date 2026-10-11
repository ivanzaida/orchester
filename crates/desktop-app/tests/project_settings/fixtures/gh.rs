//! Local fixture for project-settings account checks; no network or credentials are read.
use std::process::ExitCode;

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let output = match args.as_slice() {
    ["--version"] => "gh version 2.97.0",
    ["auth", "status", "--hostname", "github.com", "--json", "hosts"] =>
      r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"signed-in","scopes":"repo, read:org","gitProtocol":"https"}]}}"#,
    ["repo", "view", "github.com/example/project", "--json", "nameWithOwner,defaultBranchRef,viewerPermission"] =>
      r#"{"nameWithOwner":"example/project","defaultBranchRef":{"name":"main"},"viewerPermission":"ADMIN"}"#,
    _ => return ExitCode::FAILURE,
  };
  println!("{output}");
  ExitCode::SUCCESS
}
