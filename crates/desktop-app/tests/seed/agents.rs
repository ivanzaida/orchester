use orchester_workspace::agents::data::{ProjectAgents, VersionState};

use super::Seed;

#[test]
fn seed_agents_has_saved_stale_and_pipeline_draft_states() {
  let seed = Seed::of("agents");
  let listed = ProjectAgents::for_screen(seed.repo()).expect("profiles");
  let builder = listed
    .profiles
    .iter()
    .find(|p| p.profile.role().as_str() == "Builder")
    .expect("Builder");
  assert!(matches!(builder.state, VersionState::Draft { published: Some(_) }));
  assert!(listed.draft_times.contains_key(&builder.id));
  assert!(
    listed
      .profiles
      .iter()
      .any(|p| matches!(p.state, VersionState::StaleDraft { .. }))
  );
  assert!(listed.usage.steps.iter().any(|step| step.draft));
}

#[test]
fn seed_agents_empty_variants_have_no_profiles() {
  for (scenario, connections) in [("agents-empty", 1), ("agents-no-connections", 0)] {
    let seed = Seed::of(scenario);
    let listed = ProjectAgents::for_screen(seed.repo()).expect("profiles");
    assert!(listed.profiles.is_empty());
    assert_eq!(listed.connections.len(), connections);
  }
}

#[test]
fn seed_agents_failed_read_can_be_repaired_for_retry() {
  let seed = Seed::of("agents-unreadable");
  assert!(ProjectAgents::for_screen(seed.repo()).is_err());
  let db = rusqlite::Connection::open(seed.repo().join(".orchester/orchester.db")).expect("db");
  db.execute("UPDATE resource_drafts SET body = (SELECT body FROM qa_agent_draft_backup WHERE resource_id = resource_drafts.resource_id) WHERE resource_id IN (SELECT resource_id FROM qa_agent_draft_backup)", []).expect("repair");
  assert!(ProjectAgents::for_screen(seed.repo()).is_ok());
}
#[test]
fn seed_agents_exposes_warning_capabilities_and_active_override() {
  use orchester_core::{permission::CommandPermission, resource::ModelId};
  let seed = Seed::of("agents");
  let listed = ProjectAgents::for_screen(seed.repo()).expect("profiles");
  let warning = listed
    .profiles
    .iter()
    .find(|p| p.profile.role().as_str() == "CLI warning")
    .expect("warning");
  let connection = listed
    .connections
    .iter()
    .find(|c| c.id() == warning.profile.connection())
    .expect("CLI");
  assert!(
    connection
      .cannot_run(&ModelId::new("qa-unlisted-model").expect("id"))
      .is_some()
  );
  assert!(connection.models().iter().any(|m| m.efforts().len() == 3));
  assert!(
    listed
      .profiles
      .iter()
      .any(|p| matches!(p.profile.permissions().commands(), CommandPermission::Allowed(_)))
  );
  assert!(
    listed
      .profiles
      .iter()
      .any(|p| !listed.connections.iter().any(|c| c.id() == p.profile.connection()))
  );
  assert!(listed.connections.iter().any(|c| !c.provider().runs_agents()));
  assert!(listed.usage.steps.iter().any(|s| s.model_overridden && s.running == 1));
  assert!(!listed.usage.active.is_empty());
}

#[test]
fn seed_agents_library_lists_every_instruction_part_skill_scope_and_repository_file() {
  use orchester_core::{
    agent_files::import::{ItemKind, Unimportable},
    resource::InstructionScope,
  };
  use orchester_workspace::{instructions::data::ProjectInstructions, skills::data::ProjectSkills};
  let seed = Seed::of("agents-library");
  let instructions = ProjectInstructions::load_in(seed.repo()).expect("instructions");
  let entry = |title: &str| {
    instructions
      .entries
      .iter()
      .find(|entry| &*entry.name == title)
      .unwrap_or_else(|| panic!("{title}"))
  };
  assert!(matches!(
    entry("Implementation standards").state,
    VersionState::Draft { published: Some(_) }
  ));
  assert!(instructions.by_automation(entry("Commit messages")));
  assert!(matches!(
    entry("Security notes").state,
    VersionState::Draft { published: None }
  ));
  assert!(matches!(
    entry("Builder rules").body.scope(),
    InstructionScope::AgentRole { .. }
  ));
  assert!(matches!(
    entry("Feature delivery notes").body.scope(),
    InstructionScope::Pipeline { .. }
  ));
  let InstructionScope::Step { pipeline, step } = entry("Deploy checklist").body.scope() else {
    panic!("a step rule");
  };
  let feature = instructions.pipeline(*pipeline).expect("Feature delivery");
  assert!(
    !feature.steps.iter().any(|choice| choice.key == *step),
    "the rule is orphaned"
  );
  let skills = ProjectSkills::load_in(seed.repo()).expect("skills");
  let skill = |title: &str| skills.entries.iter().find(|entry| &*entry.name == title).expect(title);
  assert!(matches!(
    skill("run-checks").state,
    VersionState::Draft { published: Some(_) }
  ));
  assert!(skills.by_automation(skill("migration-guide")));
  assert!(matches!(
    skill("review-diff").body.scope(),
    InstructionScope::AgentRole { .. }
  ));
  assert!(matches!(
    skill("release-notes").body.scope(),
    InstructionScope::Pipeline { .. }
  ));
  let found = orchester_workspace::skills::repository::scan(seed.repo()).expect("the repository's agent files");
  let items: Vec<(ItemKind, &str, bool)> = found
    .items
    .iter()
    .map(|item| (item.kind, item.source.as_str(), item.outcome.is_ok()))
    .collect();
  assert_eq!(
    items,
    [
      (ItemKind::Instruction, "AGENTS.md", true),
      (ItemKind::Instruction, "CLAUDE.md", false),
      (ItemKind::Skill, ".claude/skills/write-tests", true),
      (ItemKind::Skill, ".agents/skills/drafts", false),
    ]
  );
  assert!(matches!(found.items[3].outcome, Err(Unimportable::NoSkillFile)));
}
