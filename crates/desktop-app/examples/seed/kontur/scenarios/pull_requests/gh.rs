//! The fake `gh` of the `pull-requests` scenario: a command script (`gh.cmd` on Windows, a shell script
//! elsewhere) in `.orchester/qa-bin` that answers the read-only commands a sync runs from files beside it,
//! as the real `gh` 2.97 answers them (`crates/core/tests/fixtures/github`). `gh-mode.txt` picks the answer.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  pull_request::{PullRequest, PullRequestState, ReviewDecision},
  time::Timestamp,
};
use serde_json::{Value, json};

use super::diffs;
use crate::kontur::{Kontur, SeedError};

const VERSION: &str = "gh version 2.97.0 (2026-07-31)\nhttps://github.com/cli/cli/releases/tag/v2.97.0\n";
const NOT_SIGNED_IN: &str = "You are not logged into any GitHub hosts. To log in, run: gh auth login\n";
const NETWORK: &str =
  "error connecting to api.github.com\ncheck your internet connection or https://githubstatus.com\n";
const SERVER_ERROR: &str = "HTTP 502: Server Error (https://api.github.com/graphql)\n";
const RATE_LIMITED: &str = "GraphQL: API rate limit already exceeded for user ID 1234567.\n";
/// When the fake's rate limit resets, after the seed: long enough for a QA session.
const RESET_MINUTES: i64 = 120;

/// Writes the fake and its answers for `pull_requests`, in `gh`'s order: open ones first, then closed ones.
/// Answers the program's path.
pub fn install(kontur: &Kontur, pull_requests: &[PullRequest]) -> Result<PathBuf, SeedError> {
  let bin = kontur.repo.join(".orchester").join("qa-bin");
  let answers = bin.join("gh");
  fs::create_dir_all(&answers).map_err(|error| io(&answers, error))?;
  let (open, closed): (Vec<&PullRequest>, Vec<&PullRequest>) = pull_requests
    .iter()
    .partition(|pull_request| matches!(pull_request.state(), PullRequestState::Open | PullRequestState::Draft));
  let reset = kontur.now.unix_millis() / 1000 + RESET_MINUTES * 60;
  let files = [
    ("version.txt", VERSION.to_owned()),
    ("auth.json", auth("ivanzaida", "gist, read:org, repo, workflow")),
    ("auth-other.json", auth("sam-work", "gist, read:org, repo, workflow")),
    ("auth-no-scope.json", auth("ivanzaida", "gist, read:org, workflow")),
    ("auth-none.json", "{\"hosts\":{}}\n".to_owned()),
    ("auth-none.err", NOT_SIGNED_IN.to_owned()),
    ("open.json", list(&open)),
    ("closed.json", list(&closed)),
    ("checks-open.json", checks(&open)),
    ("checks-closed.json", checks(&closed)),
    ("network.err", NETWORK.to_owned()),
    ("server.err", SERVER_ERROR.to_owned()),
    ("rate-limited.err", RATE_LIMITED.to_owned()),
    (
      "rate.json",
      json!({ "resources": { "graphql": { "reset": reset } } }).to_string(),
    ),
  ];
  for (name, contents) in files {
    write(&answers.join(name), &contents)?;
  }
  for pull_request in pull_requests {
    write(
      &answers.join(format!("view-{}.json", pull_request.number())),
      &view(pull_request),
    )?;
    let diff = diffs::diff(pull_request.number());
    write(&answers.join(format!("diff-{}.diff", pull_request.head_sha())), &diff)?;
  }
  write(&bin.join("gh-mode.txt"), "ok\n")?;
  let first_open = open
    .first()
    .map_or("none", |pull_request| pull_request.head_sha().as_str());
  program::write(&bin, first_open)
}

fn auth(login: &str, scopes: &str) -> String {
  json!({ "hosts": { "github.com": [{
    "state": "success", "active": true, "host": "github.com", "login": login,
    "tokenSource": "keyring", "scopes": scopes, "gitProtocol": "https",
  }] } })
  .to_string()
}

/// `gh pr list --json …` of `pull_requests`.
fn list(pull_requests: &[&PullRequest]) -> String {
  Value::Array(pull_requests.iter().map(|pull_request| listed(pull_request)).collect()).to_string()
}

/// A pull request as `gh pr list --json …` writes it.
fn listed(pull_request: &PullRequest) -> Value {
  let (state, draft) = match pull_request.state() {
    PullRequestState::Open => ("OPEN", false),
    PullRequestState::Draft => ("OPEN", true),
    PullRequestState::Merged => ("MERGED", false),
    PullRequestState::Closed => ("CLOSED", false),
  };
  let changes = pull_request.changes();
  json!({
    "number": pull_request.number(),
    "title": pull_request.title(),
    "url": pull_request.url(),
    "state": state,
    "isDraft": draft,
    "headRefName": pull_request.head().as_str(),
    "baseRefName": pull_request.base().as_str(),
    "headRefOid": pull_request.head_sha().as_str(),
    "author": { "login": pull_request.author() },
    "additions": changes.additions,
    "deletions": changes.deletions,
    "changedFiles": changes.files,
    "updatedAt": utc(pull_request.updated_at()),
    "reviewDecision": pull_request.review_decision().map_or("", review),
    "mergedAt": pull_request.merged_at().map(utc),
    "closedAt": pull_request.closed_at().map(utc),
    "isCrossRepository": pull_request.is_cross_repository(),
  })
}

/// `gh pr view --json …` of `pull_request`: as listed, with its checks one by one and its base revision.
fn view(pull_request: &PullRequest) -> String {
  let mut viewed = listed(pull_request);
  let counts = pull_request.checks();
  let check = |status: &str, conclusion: Option<&str>| json!({ "__typename": "CheckRun", "status": status, "conclusion": conclusion });
  let rollup: Vec<Value> = std::iter::repeat_n(check("COMPLETED", Some("SUCCESS")), counts.passed as usize)
    .chain(std::iter::repeat_n(check("IN_PROGRESS", None), counts.pending as usize))
    .chain(std::iter::repeat_n(
      check("COMPLETED", Some("FAILURE")),
      counts.failed as usize,
    ))
    .collect();
  viewed["statusCheckRollup"] = Value::Array(rollup);
  viewed["baseRefOid"] = json!(diffs::base(pull_request.number()));
  viewed.to_string()
}

const fn review(decision: ReviewDecision) -> &'static str {
  match decision {
    ReviewDecision::ReviewRequired => "REVIEW_REQUIRED",
    ReviewDecision::Approved => "APPROVED",
    ReviewDecision::ChangesRequested => "CHANGES_REQUESTED",
  }
}

/// GitHub's answer counting the checks of `pull_requests`' heads, `c<n>` for the n-th.
fn checks(pull_requests: &[&PullRequest]) -> String {
  let commits: serde_json::Map<String, Value> = pull_requests
    .iter()
    .enumerate()
    .map(|(index, pull_request)| {
      let counts = pull_request.checks();
      let rollup = (counts.total() > 0).then(|| {
        json!({ "contexts": {
          "checkRunCountsByState": [
            { "state": "SUCCESS", "count": counts.passed },
            { "state": "IN_PROGRESS", "count": counts.pending },
            { "state": "FAILURE", "count": counts.failed },
          ],
          "statusContextCountsByState": [],
        } })
      });
      let commit = json!({ "oid": pull_request.head_sha().as_str(), "statusCheckRollup": rollup });
      (format!("c{index}"), commit)
    })
    .collect();
  json!({ "data": { "repository": commits } }).to_string()
}

/// `at` as GitHub writes a time: `2026-10-15T12:04:05Z`.
fn utc(at: Timestamp) -> String {
  let seconds = at.unix_millis().div_euclid(1000);
  let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
  // Days since 1970-01-01 to a civil date (Howard Hinnant's `civil_from_days`).
  let shifted = days + 719_468;
  let era = shifted.div_euclid(146_097);
  let day_of_era = shifted.rem_euclid(146_097);
  let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
  let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  let month_index = (5 * day_of_year + 2) / 153;
  let day = day_of_year - (153 * month_index + 2) / 5 + 1;
  let month = if month_index < 10 {
    month_index + 3
  } else {
    month_index - 9
  };
  let year = year_of_era + era * 400 + i64::from(month <= 2);
  format!(
    "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
    rest / 3600,
    rest % 3600 / 60,
    rest % 60
  )
}

fn write(path: &Path, contents: &str) -> Result<(), SeedError> {
  fs::write(path, contents).map_err(|error| io(path, error))
}

fn io(path: &Path, source: std::io::Error) -> SeedError {
  SeedError::Io {
    path: path.to_path_buf(),
    source,
  }
}

/// The script itself: `cmd.exe` runs `gh.cmd` on Windows; elsewhere the system runs `gh` by its `#!` line.
#[cfg(windows)]
mod program {
  use std::path::{Path, PathBuf};

  use crate::kontur::SeedError;

  /// `first_open` is the head of the first open pull request: a check counts request that asks for it is
  /// the open list's.
  pub fn write(bin: &Path, first_open: &str) -> Result<PathBuf, SeedError> {
    let script = format!(
      "@echo off\r\nsetlocal\r\nset \"HERE=%~dp0\"\r\nset /p MODE=<\"%HERE%gh-mode.txt\"\r\nset \"A=%HERE%gh\\\"\r\n\
       if \"%~1\"==\"--version\" (type \"%A%version.txt\" & exit /b 0)\r\n\
       if \"%~1\"==\"auth\" goto auth\r\nif \"%~1 %~2\"==\"pr list\" goto list\r\n\
       if \"%~1 %~2\"==\"pr view\" goto view\r\nif \"%~2\"==\"--allow-escape-sequences\" goto compare\r\n\
       if \"%~1 %~2\"==\"api graphql\" goto checks\r\nif \"%~1\"==\"api\" (type \"%A%rate.json\" & exit /b 0)\r\n\
       exit /b 1\r\n\
       :view\r\nif \"%MODE%\"==\"unreachable\" (type \"%A%network.err\" 1>&2 & exit /b 1)\r\n\
       if \"%MODE%\"==\"server-error\" (type \"%A%server.err\" 1>&2 & exit /b 1)\r\n\
       if \"%MODE%\"==\"rate-limited\" (type \"%A%rate-limited.err\" 1>&2 & exit /b 1)\r\n\
       if exist \"%A%view-%~3.json\" (type \"%A%view-%~3.json\" & exit /b 0)\r\n\
       echo GraphQL: Could not resolve to a PullRequest with the number of %~3. (repository.pullRequest) 1>&2\r\n\
       exit /b 1\r\n\
       :compare\r\nset \"R=%~7\"\r\nset \"H=%R:*...=%\"\r\ntype \"%A%diff-%H%.diff\" & exit /b 0\r\n\
       :auth\r\nif \"%MODE%\"==\"wrong-account\" (type \"%A%auth-other.json\" & exit /b 0)\r\n\
       if \"%MODE%\"==\"missing-scope\" (type \"%A%auth-no-scope.json\" & exit /b 0)\r\n\
       if \"%MODE%\"==\"not-signed-in\" (type \"%A%auth-none.json\" & type \"%A%auth-none.err\" 1>&2 & exit /b 1)\r\n\
       type \"%A%auth.json\" & exit /b 0\r\n\
       :list\r\nif \"%MODE%\"==\"unreachable\" (type \"%A%network.err\" 1>&2 & exit /b 1)\r\n\
       if \"%MODE%\"==\"server-error\" (type \"%A%server.err\" 1>&2 & exit /b 1)\r\n\
       if \"%MODE%\"==\"rate-limited\" (type \"%A%rate-limited.err\" 1>&2 & exit /b 1)\r\n\
       if \"%~6\"==\"closed\" (type \"%A%closed.json\" & exit /b 0)\r\ntype \"%A%open.json\" & exit /b 0\r\n\
       :checks\r\nfindstr /C:\"{first_open}\" >nul && (type \"%A%checks-open.json\" & exit /b 0)\r\n\
       type \"%A%checks-closed.json\" & exit /b 0\r\n"
    );
    let path = bin.join("gh.cmd");
    std::fs::write(&path, script).map_err(|source| SeedError::Io {
      path: path.clone(),
      source,
    })?;
    Ok(path)
  }
}

#[cfg(not(windows))]
mod program {
  use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
  };

  use crate::kontur::SeedError;

  /// `first_open` is the head of the first open pull request: a check counts request that asks for it is
  /// the open list's.
  pub fn write(bin: &Path, first_open: &str) -> Result<PathBuf, SeedError> {
    let script = format!(
      "#!/bin/sh\nhere=$(dirname \"$0\")\na=\"$here/gh\"\nmode=$(cat \"$here/gh-mode.txt\")\n\
       case \"$1 $2\" in\n\
       \"--version \"*) cat \"$a/version.txt\"; exit 0;;\n\
       \"auth \"*) case \"$mode\" in\n\
         wrong-account) cat \"$a/auth-other.json\";;\n\
         missing-scope) cat \"$a/auth-no-scope.json\";;\n\
         not-signed-in) cat \"$a/auth-none.json\"; cat \"$a/auth-none.err\" >&2; exit 1;;\n\
         *) cat \"$a/auth.json\";;\n\
       esac; exit 0;;\n\
       \"pr list\") case \"$mode\" in\n\
         unreachable) cat \"$a/network.err\" >&2; exit 1;;\n\
         server-error) cat \"$a/server.err\" >&2; exit 1;;\n\
         rate-limited) cat \"$a/rate-limited.err\" >&2; exit 1;;\n\
       esac\n\
       if [ \"$6\" = closed ]; then cat \"$a/closed.json\"; else cat \"$a/open.json\"; fi; exit 0;;\n\
       \"pr view\") case \"$mode\" in\n\
         unreachable) cat \"$a/network.err\" >&2; exit 1;;\n\
         server-error) cat \"$a/server.err\" >&2; exit 1;;\n\
         rate-limited) cat \"$a/rate-limited.err\" >&2; exit 1;;\n\
       esac\n\
       if [ -f \"$a/view-$3.json\" ]; then cat \"$a/view-$3.json\"; exit 0; fi\n\
       echo \"GraphQL: Could not resolve to a PullRequest with the number of $3. (repository.pullRequest)\" >&2; \
       exit 1;;\n\
       \"api --allow-escape-sequences\") h=\"${{7##*...}}\"; cat \"$a/diff-$h.diff\"; exit 0;;\n\
       \"api graphql\") if grep -q \"{first_open}\"; then cat \"$a/checks-open.json\"; \
       else cat \"$a/checks-closed.json\"; fi; exit 0;;\n\
       \"api \"*) cat \"$a/rate.json\"; exit 0;;\n\
       esac\nexit 1\n"
    );
    let path = bin.join("gh");
    let io = |source| SeedError::Io {
      path: path.clone(),
      source,
    };
    std::fs::write(&path, script).map_err(io)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).map_err(io)?;
    Ok(path)
  }
}
