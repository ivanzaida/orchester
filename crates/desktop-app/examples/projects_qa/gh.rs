//! The stand-in `gh` of the projects QA window, built with `rustc` when the window starts: it answers the commands
//! detection runs (`gh --version`, `gh auth status`, `gh repo view`) as the GitHub state named in the file `state`
//! beside it says, and never reaches GitHub. Wrong account answers as `ivanzaida` first and as `ivan-work` after,
//! so the project expects `ivanzaida` and *Check again* finds the other account; Checking never answers.

use std::{fs, path::PathBuf, process::ExitCode, thread, time::Duration};

const REPO_VIEW: [&str; 5] = [
  "repo",
  "view",
  "github.com/ivanzaida/kontur",
  "--json",
  "nameWithOwner,defaultBranchRef,viewerPermission",
];
const AUTH_STATUS: [&str; 6] = ["auth", "status", "--hostname", "github.com", "--json", "hosts"];

fn main() -> ExitCode {
  let folder = std::env::current_exe()
    .ok()
    .and_then(|exe| exe.parent().map(PathBuf::from))
    .unwrap_or_default();
  let state = fs::read_to_string(folder.join("state")).unwrap_or_default();
  let state = state.trim();
  let args: Vec<String> = std::env::args().skip(1).collect();
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let answer = match args.as_slice() {
    ["--version"] if state == "checking" => {
      thread::sleep(Duration::from_secs(3600));
      return ExitCode::FAILURE;
    }
    ["--version"] if state == "unrecognized" => "GitHub CLI, a build Orchester does not know".to_owned(),
    ["--version"] => "gh version 2.62.0 (2024-11-14)".to_owned(),
    args if args == AUTH_STATUS => auth_status(state, &folder),
    args if args == REPO_VIEW => {
      let permission = if state == "no-access" { "NONE" } else { "ADMIN" };
      format!(
        r#"{{"nameWithOwner":"ivanzaida/kontur","defaultBranchRef":{{"name":"main"}},"viewerPermission":"{permission}"}}"#
      )
    }
    _ => return ExitCode::FAILURE,
  };
  println!("{answer}");
  ExitCode::SUCCESS
}

fn auth_status(state: &str, folder: &std::path::Path) -> String {
  if state == "not-signed-in" {
    return r#"{"hosts":{"github.com":[]}}"#.to_owned();
  }
  let asked = folder.join("asked");
  let login = if state == "wrong-account" && asked.exists() {
    "ivan-work"
  } else {
    "ivanzaida"
  };
  // Only the next answer reads it: a stand-in that cannot write it keeps answering as `ivanzaida`.
  let _kept = fs::write(&asked, "");
  let scopes = if state == "missing-scope" { "read:org" } else { "repo, read:org" };
  format!(
    r#"{{"hosts":{{"github.com":[{{"state":"success","active":true,"host":"github.com","login":"{login}","scopes":"{scopes}","gitProtocol":"https"}}]}}}}"#
  )
}
