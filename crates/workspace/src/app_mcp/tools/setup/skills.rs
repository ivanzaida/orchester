//! `list_skills` and `get_skill`: a project's skills as Skills lists and opens them (the draft if there is one,
//! otherwise the latest version), with where each applies, its versions, and the tasks whose unfinished runs
//! still hold one of them; `save_skill_draft` and `publish_skill` write them as the Skills editor does
//! ([`super::skill_input`]).

use std::path::Path;

use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::Skill,
  store::ResourceStore,
};
use orchester_store::ProjectDb;
use rmcp::{handler::server::common::schema_for_type, schemars::JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{
    Spec, Translate,
    across::{InProjects, across},
    parse, wire,
  },
  instructions::scope_json,
  profiles::state_json,
  skill_input,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  drafts::revision_of,
  links::{Link, TaskPage},
  skills::{
    data::{ProjectSkills, SkillEntry},
    uses::active,
  },
};

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "list_skills",
    description: "A project's skills, which Orchester gives agents as files they read when a task calls for them; \
                  without `project`, every project's, each naming its project (unreadable_projects lists the \
                  projects that could not be read, with why). Each: id, name, slug, description, where it applies \
                  (project, agent_role, pipeline or step), where it stands (draft, stale_draft or published, with \
                  its published version), whether its draft is the one automation saved (draft_by_automation), the \
                  repository file it was imported from, if any, and its link in the app.",
    schema: schema_for_type::<InProjects>,
    read_only: true,
  },
  Spec {
    name: "get_skill",
    description: "One skill as its editor shows it (its draft if it has one, otherwise its latest version): \
                  description, body, reference files (path and size; their content is not returned), where it \
                  applies, where it stands, whether automation saved its draft (draft_by_automation), its published \
                  versions (number and when), the tasks whose unfinished runs still use a version of it (task, \
                  version, step, link), its revision, which save_skill_draft's expected_revision takes, and its link \
                  in the app.",
    schema: schema_for_type::<InSkill>,
    read_only: true,
  },
  Spec {
    name: "save_skill_draft",
    description: "Saves a skill's draft as the Skills editor does; without `skill` a new skill is made under `name`, \
                  its slug made from the name and unique in the project. With `skill` (id, name or slug) its draft is \
                  replaced: a field left out keeps its value, and `name` renames it (its slug stays). A new skill \
                  starts with no description, an empty body, the project as where it applies and no reference \
                  files. Checked as the editor checks it: a name no other skill of the project has, ignoring case \
                  (invalid_name, name_taken); `applies_in` project, agent_role (with `role`, a role of the project's \
                  profiles), pipeline (with `pipeline`) or step (with `pipeline` and `step`, an agent step: a tool \
                  step has no agent to load it, scope_unavailable); `files` replaces the reference files, each a \
                  `path` in the skill's folder (names separated by /: no absolute path, no . or .., no drive, not \
                  SKILL.md) with its text `content`, or without content to keep the skill's file at that path; at \
                  most 100 files of at most 1024 KiB each, each path once (invalid_files). The description and body \
                  may be blank in a draft. With expected_revision, the revision get_skill or the last save returned, \
                  the draft is saved only while it is unchanged, else refused as draft_changed; without it the save \
                  goes over a change a person or another agent saved meanwhile. A draft reaches no agent, so saving \
                  is always open; the Skills page marks it as written by automation (draft_by_automation) until a \
                  person saves it or it is published. Publish it with publish_skill.",
    schema: schema_for_type::<skill_input::SaveSkillDraft>,
    read_only: false,
  },
  Spec {
    name: "publish_skill",
    description: "Publishes a skill's draft as its next version, as Publish does: runs that start from then on load \
                  it. Refused for a blank description or body (blank_description, blank_body), a draft started from \
                  an older version than the latest (stale_draft), a scope no agent loads it in any more \
                  (scope_unavailable), no draft (no_draft), and as draft_changed a draft that changed while it was \
                  checked. What it publishes reaches the prompts and files of every agent it applies to, so it is \
                  refused as person_only unless a person turned on Let automation change agent permissions, applies \
                  the Orchestrator's proposal of it, or lets the Orchestrator publish drafts; a person publishes it \
                  in the window.",
    schema: schema_for_type::<InSkill>,
    read_only: false,
  },
];

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InSkill {
  /// The project's id, or its name.
  pub project: String,
  /// The skill's id, its name or its slug.
  pub skill: String,
}

/// Calls tool `name` of [`SPECS`] with `arguments`.
pub(super) async fn call(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: Value,
) -> Result<Value, ToolError> {
  match name {
    "list_skills" => list_skills(services, parse(arguments)?, translate).await,
    "get_skill" => get_skill(services, parse(arguments)?).await,
    "save_skill_draft" => skill_input::save_skill_draft(services, parse(arguments)?).await,
    "publish_skill" => skill_input::publish_skill(services, parse(arguments)?).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

async fn list_skills(services: &Services, args: InProjects, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let read = blocking(move || across(&services, args.project.as_deref(), |_, db| Ok(ProjectSkills::load(db)?))).await?;
  Ok(read.json("skills", translate, |open, skills| {
    skills
      .entries
      .iter()
      .map(|entry| summary_json(skills, entry, open.project.id()))
      .collect()
  }))
}

async fn get_skill(services: &Services, args: InSkill) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let id = skill_named(&ProjectSkills::load_in(&open.repo_root)?, &args.skill)?.id;
    Ok(json!({ "skill": skill_json(&open.repo_root, id, open.project.id())? }))
  })
  .await
}

/// Skill `id` of `project`, at `root`, as `get_skill` shows it. Its revision is read before the skill, so a save made
/// meanwhile shows in what is returned and a save on that revision is refused rather than going over what was not
/// shown.
pub(super) fn skill_json(root: &Path, id: ResourceId, project: ProjectId) -> Result<Value, ToolError> {
  let db = ProjectDb::open(root)?;
  let revision = revision_of::<Skill>(&db, id)?;
  let skills = ProjectSkills::load(&db)?;
  let entry = skills
    .entry(id)
    .ok_or_else(|| ToolError::not_found(Missing::Skill, id.to_string()))?;
  let versions: Vec<Value> = db
    .versions::<Skill>(id)?
    .iter()
    .map(|version| json!({ "version": version.number().get(), "published_at": wire::time(version.published_at()) }))
    .collect();
  let in_use: Vec<Value> = active(&db)
    .map_err(|error| ToolError::refused("project_data", vec![error.message()]))?
    .iter()
    .filter(|used| used.skill == id)
    .map(|used| {
      json!({
        "task": &*used.task,
        "version": used.version.get(),
        "step": used.step.as_deref(),
        "link": wire::task_link(project, &used.task, TaskPage::Summary),
      })
    })
    .collect();
  let files: Vec<Value> = entry
    .body
    .files()
    .iter()
    .map(|file| json!({ "path": file.path.as_str(), "size": file.size }))
    .collect();
  let mut skill = summary_json(&skills, entry, project);
  skill["body"] = json!(entry.body.body());
  skill["files"] = json!(files);
  skill["versions"] = json!(versions);
  skill["in_use_by"] = json!(in_use);
  skill["revision"] = json!(revision);
  Ok(skill)
}

/// The project's skill named by its id or else, ignoring case, by its name or slug.
pub(super) fn skill_named<'a>(skills: &'a ProjectSkills, named: &str) -> Result<&'a SkillEntry, ToolError> {
  let lowered = named.to_lowercase();
  let by_id = skills.entries.iter().find(|entry| entry.id.to_string() == named);
  by_id
    .or_else(|| {
      skills
        .entries
        .iter()
        .find(|entry| entry.name.to_lowercase() == lowered || entry.slug.to_lowercase() == lowered)
    })
    .ok_or_else(|| ToolError::not_found(Missing::Skill, named))
}

/// A skill of `project` as `list_skills` lists it, with its link.
fn summary_json(skills: &ProjectSkills, entry: &SkillEntry, project: ProjectId) -> Value {
  json!({
    "id": entry.id.to_string(),
    "name": &*entry.name,
    "slug": &*entry.slug,
    "description": entry.body.description(),
    "applies_in": scope_json(entry.body.scope(), |id| skills.pipeline(id).map(|pipeline| &*pipeline.name)),
    "state": state_json(entry.state),
    "draft_by_automation": skills.by_automation(entry),
    "imported_from": entry.origin.as_ref().map(|origin| &origin.source),
    "link": Link::Skill(project, entry.id).path(),
  })
}
