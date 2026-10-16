//! Opening a pull request with `gh pr create -R <host/owner/name> --base <base> --head <head> --title <title>
//! --body-file - [--draft]` (`docs/design/orchester-workspace.md`, "GitHub", rule 7: a draft unless asked
//! otherwise), and what it says: the new pull request's address, or why it opened none. The body goes on
//! standard input, so free-form text never passes through a command script's arguments. What a pull request
//! of a task says is [`task_pull_request`]'s: the task's key and title, the first paragraph of its brief,
//! its acceptance criteria and where the task is in Orchester.

use std::path::Path;

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

/// What `gh` writes when the head branch already has an open pull request into the base, followed by its
/// address on the next line.
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

/// Why `gh pr create` opened no pull request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreateFailure {
  /// The head branch already has an open pull request into the base: `number`, when `gh` named it.
  AlreadyExists { number: Option<u64> },
  /// The head has no commits the base lacks.
  NoCommits,
  /// GitHub was not reached, refused the account, or `gh` failed otherwise.
  Repository(RepositoryError),
}

/// `gh pr create` of `new` on `repo`, its body on standard input.
pub fn gh_pr_create_command(executable: &Path, repo: &GitHubRepo, new: &NewPullRequest) -> Command {
  let mut args = vec![
    "pr".to_owned(),
    "create".to_owned(),
    "-R".to_owned(),
    repo.to_string(),
    "--base".to_owned(),
    new.base.to_string(),
    "--head".to_owned(),
    new.head.to_string(),
    "--title".to_owned(),
    new.title.clone(),
    "--body-file".to_owned(),
    "-".to_owned(),
  ];
  if new.draft {
    args.push("--draft".to_owned());
  }
  gh_command(executable, args).with_stdin(new.body.clone())
}

/// Reads `gh pr create` output for `repo`: the new pull request's address, the last line it prints.
pub fn parse_gh_pr_create(repo: &GitHubRepo, output: &CommandOutput) -> Result<OpenedPullRequest, CreateFailure> {
  if !output.success() {
    return Err(create_failure(repo, &output.stderr));
  }
  output
    .stdout
    .lines()
    .rev()
    .find_map(|line| pull_request_at(repo, line.trim()))
    .ok_or_else(|| CreateFailure::Repository(unrecognized(&output.stdout)))
}

fn create_failure(repo: &GitHubRepo, stderr: &str) -> CreateFailure {
  let lower = stderr.to_ascii_lowercase();
  if lower.contains(ALREADY_EXISTS) {
    let number = stderr
      .lines()
      .find_map(|line| pull_request_at(repo, line.trim()))
      .map(|opened| opened.number);
    return CreateFailure::AlreadyExists { number };
  }
  if lower.contains(NO_COMMITS) {
    return CreateFailure::NoCommits;
  }
  CreateFailure::Repository(classify_failure(repo, &redact_userinfo(stderr)))
}

/// The pull request whose address is `text`, if it is one of `repo`'s host.
fn pull_request_at(repo: &GitHubRepo, text: &str) -> Option<OpenedPullRequest> {
  let number: u64 = text.rsplit_once("/pull/")?.1.parse().ok()?;
  is_pull_request_url(repo, number, text).then(|| OpenedPullRequest {
    number,
    url: text.to_owned(),
  })
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
