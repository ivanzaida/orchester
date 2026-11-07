//! Choosing the agent for *Send to Implement* and *Ask the agent to resolve* (ORC-808): the hand-off carries the
//! profile's latest published version and model for that return only, a profile with nothing published is refused
//! before anything is kept, and no choice is the pipeline's agent. `cargo test -p orchester-workspace --test
//! review_profile`.

#[allow(dead_code, reason = "the review tests' fixture: these tests use a part of it")]
#[path = "review_threads/support.rs"]
mod support;

use std::fs;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, ResourceKind, Slug},
  run::{ChosenProfile, PinnedVersion},
  store::ResourceStore,
};
use orchester_exec::programs::Programs;
use orchester_ipc::{Command, WorkReturn};
use orchester_workspace::{
  runner::{RequestError, RunRequestError},
  tasks::{
    integration::merge_in,
    resolve::RunWorktrees,
    review_threads::{self, ReviewRefusal, conflicts, read::review_of, send},
    worktree_removal::WorktreeRemover,
  },
};
use support::{Repo, at, body, git};

const OTHER_MODEL: &str = "other-model";

fn worktrees(repo: &Repo) -> RunWorktrees {
  RunWorktrees::here_for_test(repo.root(), Programs::processes())
}

/// A profile "Fixer" on the Builder's connection with another model: published when `publish`.
fn fixer(repo: &Repo, publish: bool) -> ResourceId {
  let mut db = repo.db();
  let builder = db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .remove(0)
    .id();
  let body = db
    .latest_version::<AgentProfile>(builder)
    .expect("the builder")
    .expect("published")
    .body()
    .clone();
  let profile = AgentProfile::new(
    Name::new("Fixer").expect("name"),
    String::new(),
    PromptText::new("Fix things.").expect("prompt"),
    body.connection(),
    ModelId::new(OTHER_MODEL).expect("model"),
    ProviderSettings::default(),
  );
  let id = db
    .create_resource(
      Slug::new("fixer").expect("slug"),
      Name::new("Fixer").expect("name"),
      &profile,
      at(10),
    )
    .expect("profile")
    .id();
  if publish {
    db.publish_draft::<AgentProfile>(id, at(11)).expect("publish");
  }
  id
}

fn chosen_of(repo: &Repo, profile: ResourceId) -> ChosenProfile {
  let version = repo
    .db()
    .latest_version::<AgentProfile>(profile)
    .expect("read")
    .expect("published");
  ChosenProfile {
    profile: PinnedVersion::of(profile, &version),
    model: ModelId::new(OTHER_MODEL).expect("model"),
  }
}

fn returned(command: &Command) -> &WorkReturn {
  let Command::ReturnRun(returned) = command else {
    panic!("a return of the run's work: {command:?}");
  };
  returned
}

fn comment(repo: &Repo) {
  review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(20)).expect("kept");
}

#[test]
fn review_profile_send_to_implement_carries_the_chosen_profile_and_none_by_default() {
  let repo = Repo::new();
  comment(&repo);
  let fixer = fixer(&repo, true);
  let sending = send::begin(repo.root(), &worktrees(&repo), repo.run, Some(fixer)).expect("sending");
  assert_eq!(returned(&sending.command).profile, Some(chosen_of(&repo, fixer)));
  let not_taken = RunRequestError::Request(RequestError::NotConnected);
  send::sent(repo.root(), &sending, Err(not_taken), at(21)).expect_err("the runner did not take it");
  let again = send::begin(repo.root(), &worktrees(&repo), repo.run, None).expect("sending again");
  assert_eq!(
    returned(&again.command).profile,
    None,
    "no choice runs the pipeline's agent"
  );
}

#[test]
fn review_profile_a_profile_with_nothing_published_is_refused_and_nothing_is_kept_sending() {
  let repo = Repo::new();
  comment(&repo);
  let draft = fixer(&repo, false);
  let refused = send::begin(repo.root(), &worktrees(&repo), repo.run, Some(draft)).expect_err("nothing published");
  assert_eq!(refused, ReviewRefusal::ProfileNotPublished);
  let read = review_of(repo.root(), &Programs::processes(), repo.run).expect("read");
  assert_eq!(read.unsent(), 1, "the comment still waits unsent");
}

#[test]
fn review_profile_ask_the_agent_to_resolve_carries_the_chosen_profile() {
  let repo = Repo::new();
  fs::write(repo.root().join("src/lib.rs"), "fn one() { main }\n").expect("write");
  git(repo.root(), &["commit", "-am", "main changed it too"]);
  let remover = WorktreeRemover::here_for_test(repo.root(), Programs::processes());
  merge_in(&remover, &Programs::processes(), (repo.run, None)).expect_err("conflicts");
  let fixer = fixer(&repo, true);
  let command = conflicts::resolve::begin(repo.root(), &worktrees(&repo), repo.run, Some(fixer)).expect("the command");
  let sent = returned(&command);
  assert_eq!(sent.profile, Some(chosen_of(&repo, fixer)));
  assert!(sent.conflicts.is_some(), "the conflicts still go as data");
  let command = conflicts::resolve::begin(repo.root(), &worktrees(&repo), repo.run, None).expect("the command");
  assert_eq!(returned(&command).profile, None);
}
