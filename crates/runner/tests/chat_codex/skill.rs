//! ORC-663: a skill the person chose for a message reaches the Orchestrator on Codex as Codex's own. The runner writes
//! the chosen version as a skills folder of that skill alone, registers it with `skills/extraRoots/set`, gives Codex
//! `$<slug> <words>` with the skill named, and removes the folder once App Server is done. The person's own skills
//! (`fake-user-skills`) are disabled by path, while Codex's bundled ones stay.

use std::num::NonZeroU32;

use orchester_core::{
  conversation::skill::ChosenSkill,
  name::Name,
  resource::{InstructionScope, Skill, Slug, VersionNumber},
  store::ResourceStore,
};

use super::*;

/// Publishes `release-notes` in the project at `root` and keeps `words` with it as a message of `chat`.
fn keep_with_skill(root: &Path, chat: ChatId, words: &str) -> ChatMessageId {
  let mut db = ProjectDb::open(root).expect("open");
  let skill = Skill::new(
    "Release notes from the merged tasks.".to_owned(),
    "List the tasks merged since the last tag.".to_owned(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("a skill");
  let slug = Slug::new("release-notes").expect("a slug");
  let resource = db
    .create_resource(
      slug.clone(),
      Name::new("release-notes").expect("a name"),
      &skill,
      Timestamp::now(),
    )
    .expect("created")
    .id();
  db.publish_draft::<Skill>(resource, Timestamp::now())
    .expect("published");
  let chosen = ChosenSkill {
    resource,
    slug,
    version: VersionNumber::new(NonZeroU32::MIN),
  };
  keep(root, chat, &chosen.kept_with(&skill, words))
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_codex_loads_a_chosen_skill_as_its_own_and_keeps_the_persons_own_skills_out() {
  let project = codex();
  let root = project.repo.path();
  fs::write(root.join("fake-user-skills"), "my-own\n").expect("the person's own skills");
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let message = keep_with_skill(root, chat, "for v0.4");
  scheduler
    .answer_in_chat(message, tools())
    .await
    .expect("the turn starts");
  assert_eq!(ended(root, &mut events, message).await, TurnState::Answered);
  assert_eq!(
    replies(root, chat),
    ["Noted: $release-notes for v0.4"],
    "Codex gets the skill's mention and the person's words, not the block"
  );
  let started = log(root);
  assert!(
    started.contains("skill: release-notes loaded"),
    "the skill named in the input is one Codex registered: {started}"
  );
  assert!(!started.contains("orchester-skill"), "{started}");
  let argv = started
    .lines()
    .find_map(|line| line.strip_prefix("codex-argv: "))
    .expect("logged");
  let disabled = argv
    .split(" -c ")
    .find(|setting| setting.starts_with("skills.config=["))
    .expect("skills are disabled");
  assert!(
    disabled.contains("my-own") && disabled.contains("enabled=false"),
    "the person's own skill is disabled: {disabled}"
  );
  assert!(
    !disabled.contains("imagegen"),
    "Codex's bundled skills stay: {disabled}"
  );
  assert!(
    !disabled.contains("release-notes"),
    "the chosen skill is not: {disabled}"
  );
  assert!(
    !root
      .join(".orchester")
      .join("chat-skills")
      .join(message.to_string())
      .exists(),
    "the skill's folder is gone once App Server is done"
  );

  let plain = keep(root, chat, "And what waits?");
  scheduler.answer_in_chat(plain, tools()).await.expect("the turn starts");
  assert_eq!(ended(root, &mut events, plain).await, TurnState::Answered);
  let second = &log(root)[started.len()..];
  assert!(
    second.contains("prompt: And what waits?") && !second.contains("skill: "),
    "a message without a skill names none: {second}"
  );
}
