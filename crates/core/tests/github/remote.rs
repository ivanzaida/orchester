use std::path::Path;

use orchester_core::{
  command::CommandError,
  connection::GitHubHost,
  repository::{
    GitHubRepo, RemoteName, RepositoryError, parse_git_remotes, read_remotes, redact_userinfo, strip_userinfo,
  },
};

use crate::replay::{Replay, failed, fixture, ok};

fn repo(url: &str) -> Option<String> {
  GitHubRepo::from_remote_url(url).map(|repo| repo.to_string())
}

#[test]
fn remote_urls_name_github_repositories() {
  let orchester = Some("github.com/ivanzaida/orchester".to_owned());
  for url in [
    "https://github.com/ivanzaida/orchester.git",
    "https://github.com/ivanzaida/orchester",
    "https://github.com/ivanzaida/orchester/",
    "http://GitHub.com/ivanzaida/orchester.git",
    "https://user:secret@github.com/ivanzaida/orchester.git",
    "https://github.com:443/ivanzaida/orchester.git",
    "ssh://git@github.com/ivanzaida/orchester.git",
    "ssh://git@github.com:22/ivanzaida/orchester",
    "git@github.com:ivanzaida/orchester.git",
    "github.com:ivanzaida/orchester",
    "git@github.com:/ivanzaida/orchester.git",
  ] {
    assert_eq!(repo(url), orchester, "{url}");
  }
  assert_eq!(
    repo("git@ghe.example.com:Platform/api.v2.git").as_deref(),
    Some("ghe.example.com/Platform/api.v2")
  );
}

#[test]
fn other_urls_name_no_repository() {
  for url in [
    "C:/backups/orchester.git",
    "C:\\backups\\orchester.git",
    "/srv/git/orchester.git",
    "./relative:path/orchester",
    "file:///srv/git/orchester.git",
    "https://github.com/ivanzaida",
    "https://github.com/ivanzaida/orchester/tree/master",
    "https://github.com/-bad/orchester",
    "",
  ] {
    assert_eq!(repo(url), None, "{url}");
  }
}

#[test]
fn credentials_are_removed_from_urls_and_text() {
  assert_eq!(
    strip_userinfo("https://x-access-token:not-a-real-token@github.com/o/r.git"),
    "https://github.com/o/r.git"
  );
  assert_eq!(strip_userinfo("git@github.com:o/r.git"), "github.com:o/r.git");
  assert_eq!(strip_userinfo("C:/backups/r.git"), "C:/backups/r.git");
  let text = "fatal: unable to access 'https://user:not-a-real-token@github.com/o/r.git/': 403";
  let redacted = redact_userinfo(text);
  assert_eq!(
    redacted,
    "fatal: unable to access 'https://***@github.com/o/r.git/': 403"
  );
  assert_eq!(
    redact_userinfo("see https://github.com/o/r"),
    "see https://github.com/o/r"
  );
}

#[test]
fn git_remote_output_is_read() {
  let remotes = parse_git_remotes(fixture!("git_remote_orchester.stdout"));
  assert_eq!(remotes.len(), 1);
  assert_eq!(remotes[0].name().as_str(), "origin");
  assert_eq!(
    remotes[0].github_repo().map(|repo| repo.to_string()).as_deref(),
    Some("github.com/ivanzaida/orchester")
  );
  let lurq = parse_git_remotes(fixture!("git_remote_lurq.stdout"));
  assert_eq!(lurq[0].url(), "https://github.com/ivanzaida/lurq");
}

#[test]
fn mixed_remotes_keep_no_credentials() {
  // Synthetic: a local backup, an scp-style fork, GitLab, credentials in the URL, and ssh://.
  let remotes = parse_git_remotes(fixture!("git_remote_mixed.stdout"));
  let names: Vec<&str> = remotes.iter().map(|remote| remote.name().as_str()).collect();
  assert_eq!(names, ["backup", "fork", "gitlab", "origin", "upstream"]);
  assert!(remotes.iter().all(|remote| !remote.url().contains("not-a-real-token")));
  assert!(!format!("{remotes:?}").contains("not-a-real-token"));
  let repos: Vec<Option<String>> = remotes
    .iter()
    .map(|remote| remote.github_repo().map(|repo| repo.to_string()))
    .collect();
  assert_eq!(
    repos,
    [
      None,
      Some("github.com/someone/orchester".to_owned()),
      Some("gitlab.com/ivanzaida/orchester".to_owned()),
      Some("github.com/ivanzaida/orchester".to_owned()),
      Some("github.com/Upstream-Org/orchester".to_owned()),
    ]
  );
}

#[test]
fn reading_remotes_runs_git_in_the_repository() {
  let root = std::env::current_dir().expect("current directory");
  let runner = Replay::new([("remote -v", Ok(ok(fixture!("git_remote_orchester.stdout"))))]);
  let remotes = read_remotes(&runner, Path::new("git"), &root).expect("remotes");
  assert_eq!(remotes[0].name(), &RemoteName::origin());
  let ran = runner.ran();
  assert_eq!(ran[0].current_dir(), Some(root.as_path()));

  let not_a_repository = Replay::new([(
    "remote -v",
    Ok(failed(
      "fatal: not a git repository (or any of the parent directories): .git\n",
    )),
  )]);
  assert_eq!(
    read_remotes(&not_a_repository, Path::new("git"), &root),
    Err(RepositoryError::NotAGitRepository)
  );
  let missing = Replay::new([("remote -v", Err(CommandError::NotFound))]);
  assert!(matches!(
    read_remotes(&missing, Path::new("git"), &root),
    Err(RepositoryError::Run {
      program: "git",
      source: CommandError::NotFound
    })
  ));
  let leaky = Replay::new([(
    "remote -v",
    Ok(failed("fatal: bad config for 'https://u:not-a-real-token@github.com'")),
  )]);
  let error = read_remotes(&leaky, Path::new("git"), &root).expect_err("git failed");
  assert!(!error.to_string().contains("not-a-real-token"), "{error}");
}

#[test]
fn repository_names_are_validated() {
  let github = GitHubHost::github_com();
  assert!(GitHubRepo::new(github.clone(), "ivanzaida", ".github").is_ok());
  for (owner, name) in [
    ("", "r"),
    ("o", ""),
    ("o/x", "r"),
    ("o", ".."),
    ("o", "r s"),
    ("-o", "r"),
  ] {
    assert!(GitHubRepo::new(github.clone(), owner, name).is_err(), "{owner}/{name}");
  }
  assert!(RemoteName::new("-origin").is_err());
  assert!(RemoteName::new("my remote").is_err());
}
