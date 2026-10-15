//! `list_skills` and `get_skill`: a project's skills as Skills lists and opens them (the draft if there is one,
//! otherwise the latest version), with where each applies, its versions, and the tasks whose unfinished runs
//! still hold one of them.

use orchester_core::{id::ProjectId, resource::Skill, store::ResourceStore};
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
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
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
                  its published version), the repository file it was imported from, if any, and its link in the \
                  app.",
    schema: schema_for_type::<InProjects>,
    read_only: true,
  },
  Spec {
    name: "get_skill",
    description: "One skill as its editor shows it (its draft if it has one, otherwise its latest version): \
                  description, body, reference files (path and size; their content is not returned), where it \
                  applies, where it stands, its published versions (number and when), the tasks whose unfinished \
                  runs still use a version of it (task, version, step, link), and its link in the app.",
    schema: schema_for_type::<InSkill>,
    read_only: true,
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
    let project = open.project.id();
    let db = ProjectDb::open(&open.repo_root)?;
    let skills = ProjectSkills::load(&db)?;
    let entry = skill_named(&skills, &args.skill)?;
    let versions: Vec<Value> = db
      .versions::<Skill>(entry.id)?
      .iter()
      .map(|version| json!({ "version": version.number().get(), "published_at": wire::time(version.published_at()) }))
      .collect();
    let in_use: Vec<Value> = active(&db)
      .map_err(|error| ToolError::refused("project_data", vec![error.message()]))?
      .iter()
      .filter(|used| used.skill == entry.id)
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
    Ok(json!({ "skill": skill }))
  })
  .await
}

/// The project's skill named by its id or else, ignoring case, by its name or slug.
fn skill_named<'a>(skills: &'a ProjectSkills, named: &str) -> Result<&'a SkillEntry, ToolError> {
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
    "imported_from": entry.origin.as_ref().map(|origin| &origin.source),
    "link": Link::Skill(project, entry.id).path(),
  })
}
