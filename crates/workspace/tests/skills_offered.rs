//! The skills a composer's slash menu offers (ORC-325): the published ones at their latest version, never a draft, and
//! which agents can take one.
use std::fs;

use orchester_core::{
  connection::AgentProvider,
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::{InstructionScope, Skill},
};
use orchester_store::ProjectDb;
use orchester_workspace::skills::{
  editor,
  offered::{published_in, taken_by},
};

fn project() -> tempfile::TempDir {
  let repo = tempfile::tempdir().expect("project");
  fs::create_dir(repo.path().join(".git")).expect("git");
  ProjectDb::create(
    repo.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("key"),
      Name::new("Skills").expect("name"),
    ),
  )
  .expect("db");
  repo
}

fn skill(description: &str) -> Skill {
  Skill::new(description.into(), "Body".into(), Vec::new(), InstructionScope::Project).expect("skill")
}

#[test]
fn skills_offered_are_the_published_ones_at_their_latest_version_never_a_draft() {
  let repo = project();
  let root = repo.path();
  editor::save_in(root, None, "draft-only", &skill("Never published")).expect("draft");
  let notes = editor::save_in(root, None, "release-notes", &skill("Release notes v1")).expect("draft");
  let notes = editor::publish_in(root, &notes).expect("v1");
  let notes = editor::save_in(root, Some(&notes), "release-notes", &skill("Release notes v2")).expect("draft");
  let notes = editor::publish_in(root, &notes).expect("v2");
  editor::save_in(root, Some(&notes), "release-notes", &skill("Not published yet")).expect("draft over v2");
  let checks = editor::save_in(root, None, "checks", &skill("  Checks to run  ")).expect("draft");
  editor::publish_in(root, &checks).expect("v1");

  let offered = published_in(root).expect("read");
  let shown: Vec<(&str, &str, u32)> = offered
    .iter()
    .map(|skill| (&*skill.slug, &*skill.description, skill.version.get()))
    .collect();
  assert_eq!(
    shown,
    [("checks", "Checks to run", 1), ("release-notes", "Release notes v2", 2)],
    "in the library's order, at the latest published version; a draft never shows"
  );
}

#[test]
fn skills_are_taken_by_every_agent_this_version_runs() {
  assert!(taken_by(&[]));
  assert!(taken_by(&[
    AgentProvider::ClaudeCodeCli,
    AgentProvider::CodexCli,
    AgentProvider::AnthropicApi
  ]));
  assert!(!taken_by(&[AgentProvider::ClaudeCodeCli, AgentProvider::OpenAiApi]));
}
