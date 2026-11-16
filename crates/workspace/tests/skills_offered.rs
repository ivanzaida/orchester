//! The skills a composer's slash menu offers (ORC-325): the published ones at their latest version, never a draft, and
//! which agents can take one; a chosen one kept with its message at the version chosen (ORC-634).
use std::{fs, num::NonZeroU32};

use orchester_core::{
  connection::AgentProvider,
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  resource::{InstructionScope, Skill, VersionNumber},
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  mentions::facts,
  skills::{
    editor,
    offered::{OfferedSkill, kept_with, published_in, taken_by},
  },
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
  // A new skill is created as its first version (ORC-816): a draft is one over a version.
  let notes = editor::save_in(root, None, "release-notes", &skill("Release notes v1")).expect("v1");
  let notes = editor::save_in(root, Some(&notes), "release-notes", &skill("Release notes v2")).expect("draft");
  let notes = editor::publish_in(root, &notes).expect("v2");
  editor::save_in(root, Some(&notes), "release-notes", &skill("Not published yet")).expect("draft over v2");
  editor::save_in(root, None, "checks", &skill("  Checks to run  ")).expect("v1");

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

/// A chosen skill is kept with the message at the version the chip named, after whatever Orchester added to the words
/// (the facts of a mention); a conversation reads the chip back and shows the words alone. A version the project no
/// longer has is refused, with what to do.
#[test]
fn a_chosen_skill_is_kept_with_its_message_at_the_version_chosen() {
  let repo = project();
  let root = repo.path();
  let notes = editor::save_in(root, None, "release-notes", &skill("Release notes v1")).expect("v1");
  let notes = editor::save_in(root, Some(&notes), "release-notes", &skill("Release notes v2")).expect("draft");
  editor::publish_in(root, &notes).expect("v2");
  let mut offered = published_in(root).expect("read").remove(0);
  offered.version = VersionNumber::FIRST;
  let chosen = offered.chosen().expect("a slug");

  let db = ProjectDb::open(root).expect("db");
  let words = "for v0.4\n\n<orchester-mentions>\nfacts\n</orchester-mentions>";
  let kept = kept_with(&db, &chosen, words).expect("kept");
  assert!(
    kept.contains("description: \"Release notes v1\""),
    "the version chosen: {kept}"
  );
  assert!(kept.ends_with(words), "{kept}");
  assert_eq!(facts::said(&kept), "for v0.4", "a conversation shows the words alone");
  let chip = OfferedSkill::carried_by(&kept).expect("the chip");
  assert_eq!((&*chip.slug, chip.version), ("release-notes", VersionNumber::FIRST));

  let mut gone = chosen;
  gone.version = VersionNumber::new(NonZeroU32::new(9).expect("not zero"));
  let refused = kept_with(&db, &gone, "for v0.4").expect_err("no such version");
  assert_eq!(refused.key, "skills.chosenGone");
}
