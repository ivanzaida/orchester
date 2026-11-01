//! `save_skill_draft` and `publish_skill`: the Skills editor through Orchester's MCP server, through the editor's own
//! checked writes and rules (`skills::editor`): its name, scope and reference file rules, a save only over the draft
//! that was read, and Publish only of a current draft with a description and a body. A draft saved here is marked as
//! automation's on the Skills page; publishing it reaches agents' prompts and files, so it is a person's decision
//! ([`widening::prompt_allowed`], ORC-30).

use std::path::Path;

use orchester_core::{
  resource::{InstructionScope, PublishError, Skill},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  instructions::{AppliesIn, pipeline_named, role_named, step_named},
  profiles::no_draft,
  skills::{InSkill, skill_json, skill_named},
  widening,
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  drafts::revision_of,
  prompt_changes,
  skills::{
    data::{ProjectSkills, SkillEntry},
    editor::{self, EditorData, FileText, SkillForm, WriteError},
  },
  tasks::outside::Outside,
};

/// On an existing skill, a field left out keeps its value; on a new one it takes the default named.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SaveSkillDraft {
  /// The project's id, or its name.
  pub project: String,
  /// The skill to change, by id, name or slug; left out, a new one is made.
  pub skill: Option<String>,
  /// A new skill's name, which its slug is made from; with `skill`, its new name (the slug stays).
  pub name: Option<String>,
  /// What an agent reads to decide whether to load it; a new skill's default is none.
  pub description: Option<String>,
  /// The Markdown of its SKILL.md; a new skill's default is empty.
  pub body: Option<String>,
  /// Where it applies; a new skill's default is project.
  pub applies_in: Option<AppliesIn>,
  /// With agent_role: a role of the project's profiles.
  pub role: Option<String>,
  /// With pipeline or step: the pipeline, by id or name.
  pub pipeline: Option<String>,
  /// With step: an agent step of that pipeline, by key or name.
  pub step: Option<String>,
  /// Every reference file the skill has, replacing those it has; a new skill's default is none.
  pub files: Option<Vec<SkillFileText>>,
  /// The skill's `revision` as get_skill or the last save returned it: the draft is saved only while it is
  /// unchanged, else refused as draft_changed. Left out, the save goes over a change saved meanwhile.
  pub expected_revision: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkillFileText {
  /// Where it sits in the skill's folder, e.g. references/checks.md: names separated by /, never absolute, never .
  /// or .., never SKILL.md.
  pub path: String,
  /// Its text, at most 1024 KiB; left out, the skill's file at this path is kept as it is.
  pub content: Option<String>,
}

pub async fn save_skill_draft(services: &Services, args: SaveSkillDraft) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let root = open.repo_root.clone();
    let skills = ProjectSkills::load_in(&root)?;
    let before = args
      .skill
      .as_deref()
      .map(|named| skill_named(&skills, named))
      .transpose()?;
    if before.is_none() && args.expected_revision.is_some() {
      return Err(ToolError::InvalidArguments(
        "expected_revision names a skill's draft; a new skill has none".into(),
      ));
    }
    let name = args
      .name
      .as_deref()
      .or_else(|| before.map(|entry| &*entry.name))
      .unwrap_or_default();
    let body = skill(&root, &args, (&skills, before), name)?;
    if let (Some(entry), Some(expected)) = (before, args.expected_revision.as_deref()) {
      // Read after the skill the save goes over: a save made since is another revision.
      if revision_of::<Skill>(&ProjectDb::open(&root)?, entry.id)? != expected {
        return Err(StoreError::DraftChanged(entry.id).into());
      }
    }
    let now = Timestamp::now();
    let saved = editor::save_checked_in(&root, (before, name, &body), now).map_err(|error| write_error(&error))?;
    editor::mark_automation_in(&root, &saved, now).map_err(|error| write_error(&error))?;
    services.changes.record(open.project.id(), Outside::Skills);
    let mut skill = skill_json(&root, saved.id, open.project.id())?;
    skill["draft_kept"] = json!(saved.state.has_draft());
    Ok(json!({ "skill": skill }))
  })
  .await
}

pub async fn publish_skill(services: &Services, args: InSkill) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let skills = ProjectSkills::load(&db)?;
    let entry = skill_named(&skills, &args.skill)?;
    services.draft_decided::<Skill>(&db, entry.id)?;
    if entry.state.has_draft() {
      widening::prompt_allowed(&services, vec![prompt_changes::skill()])?;
    }
    let published = editor::publish_checked_in(&open.repo_root, entry).map_err(|error| write_error(&error))?;
    services.changes.record(open.project.id(), Outside::Skills);
    Ok(json!({ "skill": skill_json(&open.repo_root, published.id, open.project.id())? }))
  })
  .await
}

/// The skill the editor's fields `args` make over `before` (none for a new skill), named `name`, its new reference
/// files stored at `root`; refused as the editor refuses it.
fn skill(
  root: &Path,
  args: &SaveSkillDraft,
  (skills, before): (&ProjectSkills, Option<&SkillEntry>),
  name: &str,
) -> Result<Skill, ToolError> {
  let scope = scope(skills, args, before)?;
  let kept = before.map_or(&[][..], |entry| entry.body.files());
  let files = match &args.files {
    Some(given) => {
      let given: Vec<FileText<'_>> = given
        .iter()
        .map(|file| FileText {
          path: &file.path,
          content: file.content.as_deref(),
        })
        .collect();
      editor::files_in(root, &given, kept).map_err(|error| write_error(&error))?
    }
    None => kept.to_vec(),
  };
  let data = EditorData::stored_in(root).map_err(|reason| ToolError::refused("project_data", vec![reason]))?;
  let field = |given: &Option<String>, kept: fn(&Skill) -> &str| {
    given
      .as_deref()
      .or_else(|| before.map(|entry| kept(&entry.body)))
      .unwrap_or_default()
      .to_owned()
  };
  let (description, body) = (
    field(&args.description, Skill::description),
    field(&args.body, Skill::body),
  );
  SkillForm {
    name,
    description: &description,
    body: &body,
    files,
    scope,
  }
  .checked(&data, before.map(|entry| entry.id))
  .map_err(|error| write_error(&error))
}

/// Where the skill applies as `args` choose it from what the editor offers; left out, where `before` applies (a new
/// skill: the project).
fn scope(
  skills: &ProjectSkills,
  args: &SaveSkillDraft,
  before: Option<&SkillEntry>,
) -> Result<InstructionScope, ToolError> {
  fn required<'a>(given: &'a Option<String>, field: &str) -> Result<&'a str, ToolError> {
    given
      .as_deref()
      .ok_or_else(|| ToolError::InvalidArguments(format!("{field}: required where the skill applies in it")))
  }
  let Some(applies_in) = args.applies_in else {
    if args.role.is_some() || args.pipeline.is_some() || args.step.is_some() {
      return Err(ToolError::InvalidArguments(
        "role, pipeline and step choose where the skill applies: give them with applies_in".into(),
      ));
    }
    return Ok(before.map_or(InstructionScope::Project, |entry| entry.body.scope().clone()));
  };
  Ok(match applies_in {
    AppliesIn::Project => InstructionScope::Project,
    AppliesIn::AgentRole => InstructionScope::AgentRole {
      role: role_named(skills.roles(), required(&args.role, "role")?)?,
    },
    AppliesIn::Pipeline => InstructionScope::Pipeline {
      pipeline: pipeline_named(&skills.pipelines, required(&args.pipeline, "pipeline")?)?.id,
    },
    AppliesIn::Step => {
      let pipeline = pipeline_named(&skills.pipelines, required(&args.pipeline, "pipeline")?)?;
      InstructionScope::Step {
        pipeline: pipeline.id,
        step: step_named(pipeline, required(&args.step, "step")?)?,
      }
    }
  })
}

fn write_error(error: &WriteError) -> ToolError {
  let code = match error {
    WriteError::Refused(refusal, _) => refusal.code(),
    WriteError::Store(StoreError::NoDraft(_)) => return no_draft(),
    WriteError::Store(StoreError::DraftChanged(_)) => "draft_changed",
    WriteError::Store(StoreError::Publish(PublishError::StaleDraft { .. })) => "stale_draft",
    WriteError::Store(StoreError::Publish(PublishError::VersionLimit)) => "version_limit",
    WriteError::Store(StoreError::NotAdmin) => "not_admin",
    WriteError::Unread(_) | WriteError::Store(_) => "project_data",
  };
  ToolError::refused(code, vec![error.message()])
}
