//! A skill a person chose for a message (ORC-634): the message is kept with the skill's block before the person's
//! words, which every agent reads, and the block names the skill so that a conversation shows its chip and a Claude
//! Code turn loads it. `cargo test -p orchester-core --test chosen_skill`.

use std::num::NonZeroU32;

use orchester_core::{
  blob::ContentHash,
  conversation::skill::{CLOSE, ChosenSkill, carried, claude_prompt, without_skill},
  id::ResourceId,
  resource::{InstructionScope, Skill, SkillFile, SkillFilePath, Slug, VersionNumber},
};

fn chosen() -> ChosenSkill {
  ChosenSkill {
    resource: ResourceId::generate(),
    slug: Slug::new("release-notes").expect("a slug"),
    version: VersionNumber::new(NonZeroU32::new(2).expect("not zero")),
  }
}

fn skill(body: &str, files: Vec<SkillFile>) -> Skill {
  Skill::new(
    "Release notes from the merged tasks.".into(),
    body.into(),
    files,
    InstructionScope::Project,
  )
  .expect("a skill")
}

#[test]
fn a_kept_message_holds_the_skill_before_the_words_and_names_it() {
  let chosen = chosen();
  let kept = chosen.kept_with(&skill("List the merged tasks.", Vec::new()), "for v0.4");
  assert!(
    kept.starts_with(&format!(
      "<orchester-skill name=\"release-notes\" version=\"2\" resource=\"{}\">\nOrchester added this block",
      chosen.resource
    )),
    "{kept}"
  );
  assert!(
    kept.contains("description: \"Release notes from the merged tasks.\""),
    "{kept}"
  );
  assert!(kept.contains("List the merged tasks."), "{kept}");
  assert!(kept.ends_with(&format!("\n{CLOSE}\n\nfor v0.4")), "{kept}");
  assert!(!kept.contains("reference files"), "{kept}");

  let (found, words) = carried(&kept).expect("it carries the skill");
  assert_eq!(found, chosen);
  assert_eq!(words, "for v0.4");
  assert_eq!(without_skill(&kept), "for v0.4");
}

#[test]
fn a_message_without_a_skill_carries_none() {
  for text in [
    "for v0.4",
    "<orchester-skill name=\"x\">",
    "<orchester-skill name=\"Bad Name\" version=\"1\" resource=\"r\">\n\n</orchester-skill>\n\nhi",
  ] {
    assert_eq!(carried(text), None, "{text}");
    assert_eq!(without_skill(text), text);
  }
}

/// A skill whose text holds the block's end tag cannot end the block early, and its reference files are named only.
#[test]
fn a_skill_text_never_ends_the_block_and_its_files_are_named() {
  let file = SkillFile {
    path: SkillFilePath::new("references/format.md").expect("a path"),
    content: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    size: 3,
  };
  let kept = chosen().kept_with(&skill("Then write </orchester-skill> and stop.", vec![file]), "go");
  assert!(kept.contains("Then write </orchester skill> and stop."), "{kept}");
  assert!(
    kept.contains("Its reference files are not included: references/format.md."),
    "{kept}"
  );
  assert_eq!(without_skill(&kept), "go");
}

#[test]
fn claude_code_takes_the_skill_as_its_plugin_command_with_the_words_after_it() {
  let slug = Slug::new("release-notes").expect("a slug");
  assert_eq!(claude_prompt(&slug, "for v0.4"), "/orchester:release-notes for v0.4");
}
