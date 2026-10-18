//! Opening a pull request with `gh api --method POST --hostname <host> repos/<owner>/<name>/pulls --input -`
//! (`docs/design/orchester-workspace.md`, "GitHub", rule 7: a draft unless asked otherwise), and what GitHub
//! answers: the new pull request, or why it opened none. The title, body, branches and draft choice are the
//! request's JSON body on standard input, never arguments: a `gh` started through a `.cmd` script only gets
//! arguments `cmd.exe` passes on unchanged, and a title such as `KEY · Title` is not one. What a pull request
//! of a task says is [`task_pull_request`]'s: the task's key and title, the first paragraph of its brief, its
//! acceptance criteria and where the task is in Orchester.

use std::path::Path;

use serde::Deserialize;

use super::is_pull_request_url;
use crate::{
  command::{Command, CommandOutput},
  connection::gh_command,
  id::ProjectId,
  project::TaskKey,
  repository::{GitHubRepo, RepositoryError, classify_failure, redact_userinfo, unrecognized},
  run::BranchName,
  task::TaskDetails,
};

/// What GitHub says when the head branch already has an open pull request into the base (`A pull request
/// already exists for owner:branch.`).
const ALREADY_EXISTS: &str = "already exists";
/// What GitHub says when the head has no commits the base lacks.
const NO_COMMITS: &str = "no commits between";

/// A pull request to open from `head` into `base`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPullRequest {
  pub base: BranchName,
  pub head: BranchName,
  pub title: String,
  pub body: String,
  /// Opens as a draft, not ready for review yet.
  pub draft: bool,
}

/// A pull request GitHub opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenedPullRequest {
  pub number: u64,
  /// Its page on the repository's host.
  pub url: String,
}

/// Why GitHub opened no pull request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreateFailure {
  /// The head branch already has an open pull request into the base: `number`, when GitHub named it.
  AlreadyExists { number: Option<u64> },
  /// The head has no commits the base lacks.
  NoCommits,
  /// GitHub was not reached, refused the account, or `gh` failed otherwise.
  Repository(RepositoryError),
}

/// The fields of GitHub's answer Orchester reads.
#[derive(Deserialize)]
struct Answer {
  number: u64,
  html_url: String,
}

/// The REST request opening `new` on `repo`, with `new` as its body on standard input.
pub fn gh_pr_create_command(executable: &Path, repo: &GitHubRepo, new: &NewPullRequest) -> Command {
  let body = serde_json::json!({
    "title": new.title,
    "body": new.body,
    "head": new.head.as_str(),
    "base": new.base.as_str(),
    "draft": new.draft,
  });
  let path = format!("repos/{}/{}/pulls", repo.owner(), repo.name());
  let args = [
    "api",
    "--method",
    "POST",
    "--hostname",
    repo.host().as_str(),
    &path,
    "--input",
    "-",
  ];
  gh_command(executable, args.map(str::to_owned).to_vec()).with_stdin(body.to_string())
}

/// Reads GitHub's answer to [`gh_pr_create_command`] for `repo`: the new pull request's number and page.
pub fn parse_gh_pr_create(repo: &GitHubRepo, output: &CommandOutput) -> Result<OpenedPullRequest, CreateFailure> {
  if !output.success() {
    return Err(create_failure(repo, output));
  }
  let answer: Answer = serde_json::from_str(output.stdout.trim())
    .map_err(|error| CreateFailure::Repository(unrecognized(&error.to_string())))?;
  if !is_pull_request_url(repo, answer.number, &answer.html_url) {
    return Err(CreateFailure::Repository(unrecognized(&answer.html_url)));
  }
  Ok(OpenedPullRequest {
    number: answer.number,
    url: answer.html_url,
  })
}

/// Why GitHub refused: `gh api` writes the answer's `message` to standard error (`gh: Validation Failed (HTTP
/// 422)`) and the answer itself to standard output, whose `errors` say what failed validation.
fn create_failure(repo: &GitHubRepo, output: &CommandOutput) -> CreateFailure {
  let details = error_messages(&output.stdout);
  let lower = format!("{}\n{details}", output.stderr).to_ascii_lowercase();
  if lower.contains(ALREADY_EXISTS) {
    // GitHub names the branch, not the pull request: the sync that follows finds it.
    return CreateFailure::AlreadyExists { number: None };
  }
  if lower.contains(NO_COMMITS) {
    return CreateFailure::NoCommits;
  }
  CreateFailure::Repository(classify_failure(repo, &redact_userinfo(&output.stderr)))
}

/// The `message` of each of the `errors` of GitHub's error answer `stdout`, one a line; none when it is not one.
fn error_messages(stdout: &str) -> String {
  let Ok(answer) = serde_json::from_str::<serde_json::Value>(stdout.trim()) else {
    return String::new();
  };
  answer["errors"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|error| error["message"].as_str().or_else(|| error.as_str()))
    .collect::<Vec<_>>()
    .join("\n")
}

/// Where task `key` of project `project` is in Orchester, as the app's links name it.
pub fn task_link(project: ProjectId, key: &TaskKey) -> String {
  format!("orchester:/project/{project}/tasks/{key}")
}

/// The title and body of the pull request of task `key` with `details`, found in Orchester at `link`: the key
/// and the title; the brief's first paragraph, the acceptance criteria numbered, and the task's link.
pub fn task_pull_request(key: &TaskKey, details: &TaskDetails, link: &str) -> (String, String) {
  let title = format!("{key} · {}", details.title.as_str());
  let summary = summary(&details.brief);
  let criteria: Vec<String> = details
    .acceptance_criteria
    .iter()
    .map(|criterion| criterion.trim())
    .filter(|criterion| !criterion.is_empty())
    .enumerate()
    .map(|(index, criterion)| format!("{}. {criterion}", index + 1))
    .collect();
  let criteria = (!criteria.is_empty()).then(|| format!("## Acceptance criteria\n\n{}", criteria.join("\n")));
  let parts: Vec<String> = [(!summary.is_empty()).then_some(summary), criteria]
    .into_iter()
    .flatten()
    .chain([format!("Task {key} in Orchester: {link}")])
    .collect();
  (title, parts.join("\n\n"))
}

/// The brief's first paragraph: its lines up to the first blank one.
fn summary(brief: &str) -> String {
  brief
    .trim()
    .lines()
    .map(str::trim_end)
    .take_while(|line| !line.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}
