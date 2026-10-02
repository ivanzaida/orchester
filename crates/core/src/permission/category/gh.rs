//! The GitHub CLI by its command and action (`gh pr view`): what only reads GitHub, and creating or
//! commenting on a pull request or issue, reach the network; merging a pull request and deleting a
//! repository, release, issue or run are History / destructive; `gh auth token` prints a credential.
//! Any other command (secrets, extensions, aliases, configuration, authentication, `gh repo edit`, …)
//! is Unknown, as is an option that opens a browser or an editor. `gh api` is understood only as a
//! GET without fields.

use super::programs::{Judged, Kind};

/// Commands with the actions that only read GitHub.
const READS: [(&str, &[&str]); 9] = [
  ("pr", &["view", "list", "diff", "checks", "status"]),
  ("issue", &["view", "list", "status"]),
  ("repo", &["view", "list"]),
  ("run", &["view", "list", "watch"]),
  ("workflow", &["view", "list"]),
  ("release", &["view", "list"]),
  ("label", &["list"]),
  ("search", &["repos", "issues", "prs", "code", "commits"]),
  ("cache", &["list"]),
];

/// Commands with the actions that add to GitHub without discarding anything.
const ADDS: [(&str, &[&str]); 2] = [("pr", &["create", "comment"]), ("issue", &["create", "comment"])];

/// Commands with the actions that merge or delete.
const DESTROYS: [(&str, &[&str]); 5] = [
  ("pr", &["merge"]),
  ("repo", &["delete"]),
  ("release", &["delete"]),
  ("issue", &["delete"]),
  ("run", &["delete"]),
];

/// Options that start another program: a browser or the user's editor.
const STARTS_PROGRAMS: [&str; 4] = ["-w", "--web", "-e", "--editor"];

/// `gh` with `args`.
pub(super) fn judge(args: &[String]) -> Judged {
  let Some(command) = args.first().filter(|command| !command.starts_with('-')) else {
    return Judged::unknown();
  };
  if args.iter().any(|arg| {
    STARTS_PROGRAMS.contains(&arg.as_str())
      || STARTS_PROGRAMS
        .iter()
        .any(|option| arg.starts_with(&format!("{option}=")))
  }) {
    return Judged::unknown();
  }
  match command.as_str() {
    "api" => return api(&args[1..]).subcommand(0, "api".to_owned()),
    "status" => return Judged::of(Kind::Network).subcommand(0, "status".to_owned()),
    _ => {}
  }
  let Some(action) = args.get(1).filter(|action| !action.starts_with('-')) else {
    return Judged::unknown();
  };
  let listed = |table: &[(&str, &[&str])]| {
    table
      .iter()
      .any(|(listed, actions)| listed == command && actions.contains(&action.as_str()))
  };
  let judged = if listed(&READS) || listed(&ADDS) {
    Judged::of(Kind::Network)
  } else if listed(&DESTROYS) {
    Judged::of(Kind::Destructive)
  } else if command == "auth" && action == "token" {
    Judged::unknown().secret()
  } else {
    return Judged::unknown();
  };
  judged.subcommand(0, format!("{command} {action}"))
}

/// `gh api <endpoint>` that only reads: no method but GET, no fields or input (which make it a POST), and
/// not `graphql`, whose query may change anything.
fn api(args: &[String]) -> Judged {
  let mut endpoint = None;
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    let method = match arg.as_str() {
      "-X" | "--method" => args.next().map(String::as_str),
      other => other
        .strip_prefix("--method=")
        .or_else(|| other.strip_prefix("-X"))
        .filter(|_| other.len() > 2),
    };
    if let Some(method) = method {
      if !method.eq_ignore_ascii_case("GET") {
        return Judged::unknown();
      }
      continue;
    }
    match arg.as_str() {
      "-f" | "-F" | "--field" | "--raw-field" | "--input" => return Judged::unknown(),
      "-H" | "--header" | "-q" | "--jq" | "-t" | "--template" | "--hostname" | "--cache" | "-p" | "--preview" => {
        if args.next().is_none() {
          return Judged::unknown();
        }
      }
      "--paginate" | "--slurp" | "-i" | "--include" | "--silent" | "--verbose" => {}
      other if other.starts_with('-') => return Judged::unknown(),
      other if endpoint.is_none() => endpoint = Some(other),
      _ => return Judged::unknown(),
    }
  }
  match endpoint {
    Some(endpoint) if endpoint != "graphql" => Judged::of(Kind::Network),
    _ => Judged::unknown(),
  }
}
