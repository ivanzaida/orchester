//! `agents-library`: the `agents` scenario with what the Agents hub's Instructions and Skills tabs list (`Agents ·
//! redesign` 02 `jtiqe` to 03c `cC55E`). Instructions of every prompt part: project rules (Implementation standards
//! with variables and a draft newer than v2, Commit messages whose draft automation wrote, Security notes never
//! published), Builder's role rule, a Feature delivery rule with a Review step override, and a rule left over for a
//! Deploy step the pipeline no longer has. Skills of each scope: run-checks with a draft newer than v1, Reviewer's
//! review-diff, Feature delivery's release-notes and an unpublished migration-guide automation wrote. And the
//! repository's own agent files for Import from repository: `AGENTS.md`, a `CLAUDE.md` that only imports it,
//! `.claude/skills/write-tests` with a reference file, and `.agents/skills/drafts` without a `SKILL.md`.

use std::{collections::BTreeSet, fs};

use orchester_core::{
  id::ResourceId,
  resource::{
    AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceBody, ResourceKind, Skill,
    Slug, StepKey,
  },
  store::ResourceStore,
};

use crate::kontur::{Kontur, SeedError, name};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::agents::seed(kontur)?;
  let feature = pipeline(kontur, "Feature delivery")?;
  project_rules(kontur)?;
  scoped_rules(kontur, feature)?;
  skills(kontur, feature)?;
  repository_files(kontur)
}

fn project_rules(kontur: &mut Kontur) -> Result<(), SeedError> {
  let standards = publish(
    kontur,
    "Implementation standards",
    vec![
      instruction("Keep each change small.", InstructionScope::Project)?,
      instruction(
        "Work on {{task.title}} in {{project.name}}. Keep each change small and tested.",
        InstructionScope::Project,
      )?,
    ],
  )?;
  draft(
    kontur,
    standards,
    instruction(
      "Work on {{task.title}} in {{project.name}} on {{run.branch}}. Keep each change small, tested and described.",
      InstructionScope::Project,
    )?,
    false,
  )?;
  let commits = publish(
    kontur,
    "Commit messages",
    vec![instruction(
      "Write commit messages in the imperative.",
      InstructionScope::Project,
    )?],
  )?;
  draft(
    kontur,
    commits,
    instruction(
      "Write commit messages in the imperative and name the task key.",
      InstructionScope::Project,
    )?,
    true,
  )?;
  kontur.db.create_resource(
    slug("Security notes"),
    name("Security notes"),
    &instruction("Never log secrets or tokens.", InstructionScope::Project)?,
    kontur.ago(45),
  )?;
  Ok(())
}

/// Builder's role rule, Feature delivery's rule and Review override, and a rule for a step Feature delivery lacks.
fn scoped_rules(kontur: &mut Kontur, feature: ResourceId) -> Result<(), SeedError> {
  publish(
    kontur,
    "Builder rules",
    vec![instruction(
      "Run the tests before each commit.",
      InstructionScope::AgentRole { role: name("Builder") },
    )?],
  )?;
  publish(
    kontur,
    "Feature delivery notes",
    vec![instruction(
      "Link the task in every pull request of {{pipeline.name}}.",
      InstructionScope::Pipeline { pipeline: feature },
    )?],
  )?;
  publish(
    kontur,
    "Review checklist",
    vec![instruction(
      "Check the acceptance criteria one by one.",
      InstructionScope::Step {
        pipeline: feature,
        step: step("review"),
      },
    )?],
  )?;
  // Feature delivery has no Deploy step: the rule applies to no step, and stays openable.
  publish(
    kontur,
    "Deploy checklist",
    vec![instruction(
      "Confirm the preview before deploying.",
      InstructionScope::Step {
        pipeline: feature,
        step: step("deploy"),
      },
    )?],
  )?;
  Ok(())
}

fn skills(kontur: &mut Kontur, feature: ResourceId) -> Result<(), SeedError> {
  let checks = publish(
    kontur,
    "run-checks",
    vec![skill(
      "Runs the project's checks before a commit.",
      "Run `cargo test` and fix what fails.",
      InstructionScope::Project,
    )?],
  )?;
  draft(
    kontur,
    checks,
    skill(
      "Runs the project's checks before a commit.",
      "Run `cargo clippy` and `cargo test`, then fix what fails.",
      InstructionScope::Project,
    )?,
    false,
  )?;
  publish(
    kontur,
    "review-diff",
    vec![skill(
      "Reads a diff against the acceptance criteria.",
      "List each criterion and the lines that meet it.",
      InstructionScope::AgentRole { role: name("Reviewer") },
    )?],
  )?;
  publish(
    kontur,
    "release-notes",
    vec![skill(
      "Writes release notes for people.",
      "Group the changes by what a person notices.",
      InstructionScope::Pipeline { pipeline: feature },
    )?],
  )?;
  let guide = kontur
    .db
    .create_resource(
      slug("migration-guide"),
      name("migration-guide"),
      &skill(
        "Writes a migration for a schema change.",
        "Add one numbered SQL file per change.",
        InstructionScope::Project,
      )?,
      kontur.ago(20),
    )?
    .id();
  mark_automation::<Skill>(kontur, guide)
}

/// The repository's own agent files, as Import from repository finds them in the worktree.
fn repository_files(kontur: &Kontur) -> Result<(), SeedError> {
  let files = [
    (
      "AGENTS.md",
      "# Kontur\n\nRun `cargo test` before each commit.\nKeep webhook retries idempotent.\n",
    ),
    ("CLAUDE.md", "@AGENTS.md\n"),
    (
      ".claude/skills/write-tests/SKILL.md",
      "---\nname: write-tests\ndescription: Writes a test for each fixed bug.\n---\n\nAdd the test under `tests/` \
       before the fix.\n",
    ),
    (
      ".claude/skills/write-tests/examples.md",
      "# Examples\n\nSee `tests/webhooks/retry.rs`.\n",
    ),
    (".agents/skills/drafts/notes.md", "Ideas for skills, not a skill yet.\n"),
  ];
  for (path, text) in files {
    let path = kontur.repo.join(path);
    if let Some(folder) = path.parent() {
      fs::create_dir_all(folder).map_err(|error| SeedError::Invalid(error.to_string()))?;
    }
    fs::write(&path, text).map_err(|error| SeedError::Invalid(error.to_string()))?;
  }
  Ok(())
}

/// Creates `title`'s resource with the first of `bodies` and publishes each in turn, three days ago.
fn publish<B: ResourceBody>(kontur: &mut Kontur, title: &str, bodies: Vec<B>) -> Result<ResourceId, SeedError> {
  let at = kontur.ago(3 * 24 * 60);
  let mut bodies = bodies.into_iter();
  let first = bodies.next().expect("every resource has a first version");
  let id = kontur.db.create_resource(slug(title), name(title), &first, at)?.id();
  let mut version = kontur.db.publish_draft::<B>(id, at)?;
  for body in bodies {
    kontur
      .db
      .save_draft(id, &Draft::new(body, Some(version.number()), at))?;
    version = kontur.db.publish_draft::<B>(id, at)?;
  }
  Ok(id)
}

/// Saves `body` as a draft over `id`'s latest version half an hour ago; automation's when `by_automation`.
fn draft<B: ResourceBody>(kontur: &mut Kontur, id: ResourceId, body: B, by_automation: bool) -> Result<(), SeedError> {
  let latest = kontur.db.latest_version::<B>(id)?.map(|version| version.number());
  kontur.db.save_draft(id, &Draft::new(body, latest, kontur.ago(30)))?;
  if by_automation {
    mark_automation::<B>(kontur, id)?;
  }
  Ok(())
}

/// Marks `id`'s saved draft as the one automation wrote, as `save_instruction_draft` and `save_skill_draft` do.
fn mark_automation<B: ResourceBody>(kontur: &mut Kontur, id: ResourceId) -> Result<(), SeedError> {
  let saved = kontur
    .db
    .draft::<B>(id)?
    .ok_or_else(|| SeedError::Invalid("automation's draft was not saved".into()))?
    .updated_at();
  kontur.db.mark_automation_draft(id, saved)?;
  Ok(())
}

fn pipeline(kontur: &Kontur, title: &str) -> Result<ResourceId, SeedError> {
  kontur
    .db
    .resources(ResourceKind::Pipeline)?
    .into_iter()
    .find(|resource| resource.name().as_str() == title)
    .map(|resource| resource.id())
    .ok_or_else(|| SeedError::Invalid(format!("the Agents fixture needs {title}")))
}

fn instruction(text: &str, scope: InstructionScope) -> Result<Instruction, SeedError> {
  Instruction::new(
    PromptText::new(text).map_err(|error| SeedError::Invalid(error.to_string()))?,
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    scope,
  )
  .map_err(|error| SeedError::Invalid(error.to_string()))
}

fn skill(description: &str, body: &str, scope: InstructionScope) -> Result<Skill, SeedError> {
  Skill::new(description.into(), body.into(), Vec::new(), scope).map_err(|error| SeedError::Invalid(error.to_string()))
}

fn slug(title: &str) -> Slug {
  Slug::from_name(title).expect("the seed's names have letters")
}

fn step(key: &str) -> StepKey {
  StepKey::new(key).expect("a valid step key")
}
