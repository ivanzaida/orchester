//! A team project's skills through the tools (`docs/design/local-and-team-parity.md`): they are the team's resources,
//! read from the team's configuration this session read. `list_skills` and `get_skill` read them, where a skill applies
//! is chosen among the team's profiles and pipelines, `save_skill_draft` publishes the next version on the team server
//! at once (without `skill`, the team's new skill as its version 1), checked as a local save and a publication are, its
//! new reference files sent from this project's blobs when the server asks for them, and `publish_skill` answers it as
//! it stands: the team keeps no draft on a computer. What is published reaches agents' prompts and files, so it is a
//! person's decision as a local publication is ([`widening::prompt_allowed`]). A team skill keeps its name.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  resource::{ResourceKind, Skill},
  store::SettingsStore,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{
  super::wire,
  created::refused as edit_refused,
  given_by::Givers,
  skill_input::{SaveSkillDraft, skill, write_error},
  skills::{in_use, opened, skill_named, summary_json},
  team_profiles::TeamProject,
  team_resources::{
    Sent, TeamEntry, based_on, config_read, entries, marked, named, pipeline_choices, pipelines, profile_choices, send,
  },
  widening,
};
use crate::{
  agents::data::VersionState,
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::{self, Message},
  prompt_changes,
  sharing::team_edit::{NewTeamResource, Publication},
  skills::{
    data::{ProjectSkills, SkillEntry},
    editor::EditorData,
  },
  tasks::outside::Outside,
  team_config::TeamConfig,
  workspace::OpenProject,
};

const KIND: (ResourceKind, Missing) = (ResourceKind::Skill, Missing::Skill);

/// The team's skills in `config` with the team's profiles and pipelines where they apply, as the local tools read a
/// project's own.
fn skills(config: &TeamConfig) -> ProjectSkills {
  let profiles = profile_choices(config);
  let entries = entries::<Skill>(config, ResourceKind::Skill)
    .into_iter()
    .map(|team| SkillEntry {
      id: team.id,
      name: team.name,
      slug: team.slug,
      body: team.body,
      state: VersionState::Published(team.number),
      last_published: Some(team.published_at),
      draft_saved_at: None,
      origin: None,
    })
    .collect();
  ProjectSkills {
    entries: Arc::new(entries),
    pipelines: Arc::new(pipeline_choices(config, &profiles)),
    profiles: Arc::new(profiles),
    automation_drafts: Arc::default(),
  }
}

/// The team's skills of the project at `open`, each with its id, as `list_skills` lists them; none for a local project
/// and while the team's configuration was not read this session.
pub(super) fn listed(
  services: &Services,
  (open, db): (&OpenProject, &ProjectDb),
) -> Result<Vec<(ResourceId, Value)>, ToolError> {
  let Some(config) = config_read(services, (open, db))? else {
    return Ok(Vec::new());
  };
  let (skills, givers) = (skills(&config), Givers::team(&config));
  Ok(
    skills
      .entries
      .iter()
      .map(|entry| (entry.id, shown(&skills, &givers, entry, open.project.id())))
      .collect(),
  )
}

/// `entry` as `list_skills` lists it, marked as the team's.
fn shown(skills: &ProjectSkills, givers: &Givers, entry: &SkillEntry, project: ProjectId) -> Value {
  marked(summary_json((skills, givers), entry, project), latest(entry))
}

/// The number of the version a team skill's entry holds, its latest.
fn latest(entry: &SkillEntry) -> orchester_core::resource::VersionNumber {
  entry.state.published().expect("a team skill is its latest version")
}

/// The team skill `named` of the team project `team`, when the team has one by that id, name or slug.
pub(super) fn of(team: &TeamProject, named: &str) -> Option<ResourceId> {
  self::named::<Skill>(&team.config, KIND, named)
    .ok()
    .map(|entry| entry.id)
}

/// Team skill `id` of `open`'s project as `get_skill` answers it, read now, with its latest version.
pub(super) async fn shown_now(services: &Services, open: &OpenProject, id: ResourceId) -> Result<Value, ToolError> {
  let (services, open) = (services.clone(), open.clone());
  blocking(move || {
    let config = services.workspace.team_read(&open.repo_root).loaded()?;
    let team: TeamEntry<Skill> = named(&config, KIND, &id.to_string())?;
    let skills = skills(&config);
    let entry = skills
      .entry(id)
      .ok_or_else(|| ToolError::not_found(Missing::Skill, id.to_string()))?;
    let project = open.project.id();
    let mut skill = shown(&skills, &Givers::team(&config), entry, project);
    let versions = vec![json!({ "version": team.number.get(), "published_at": wire::time(team.published_at) })];
    let used = in_use(&ProjectDb::open(&open.repo_root)?, id, project)?;
    opened(&mut skill, &entry.body, (versions, used));
    Ok(json!({ "skill": skill }))
  })
  .await
}

/// Publishes the skill `args` give in the team project `team` as its next version, or as the team's new skill; the
/// skill as `get_skill` shows it after.
pub(super) async fn save(services: &Services, team: &TeamProject, args: SaveSkillDraft) -> Result<Value, ToolError> {
  let (reading, config, root) = (services.clone(), team.config.clone(), team.open.repo_root.clone());
  let sent = blocking(move || prepared(&reading, (&root, &config), &args)).await?;
  let id = send(services, &team.open.repo_root, sent).await?;
  services.changes.record(team.open.project.id(), Outside::Skills);
  let mut shown = shown_now(services, &team.open, id).await?;
  shown["skill"]["draft_kept"] = json!(false);
  Ok(shown)
}

/// What a save of `args` sends, checked as a local save and a publication are, and refused while publishing it is a
/// person's decision. Blocks on SQLite.
fn prepared(
  services: &Services,
  (root, config): (&Path, &TeamConfig),
  args: &SaveSkillDraft,
) -> Result<Sent<Skill>, ToolError> {
  let agent_files = ProjectDb::open(root)?.repository_agent_files()?;
  let data = EditorData::checking(skills(config), &pipelines(config), agent_files);
  let before = args
    .skill
    .as_deref()
    .map(|named| skill_named(&data.listed, named))
    .transpose()?;
  let name = match before {
    Some(entry) => {
      based_on((entry.id, latest(entry)), args.expected_revision.as_deref())?;
      if args.name.as_deref().is_some_and(|name| name != &*entry.name) {
        let reason = Message::new("appMcp.errors.teamSkillNameKept").with("skill", entry.name.to_string());
        return Err(ToolError::refused("name_not_editable", vec![reason]));
      }
      &*entry.name
    }
    None if args.expected_revision.is_some() => {
      return Err(ToolError::InvalidArguments(
        "expected_revision names a skill's version; a new skill has none".into(),
      ));
    }
    None => args.name.as_deref().unwrap_or_default(),
  };
  let body = skill(root, args, (&data, before), name)?;
  data
    .check_publication(before.map(|entry| &entry.body), &body)
    .map_err(|error| write_error(&error))?;
  // Every save publishes at once, so it reaches agents' prompts and files as a local publication does.
  widening::prompt_allowed(services, vec![prompt_changes::skill()])?;
  if let Some(entry) = before {
    return Ok(Sent::Next(entry.id, latest(entry), body));
  }
  let name = Name::new(name).map_err(|error| ToolError::refused("invalid_name", vec![messages::name_error(&error)]))?;
  let id = ResourceId::generate();
  let publication = Publication::of_body(root, id, &body).map_err(|error| edit_refused(&error))?;
  Ok(Sent::New(NewTeamResource {
    id,
    kind: ResourceKind::Skill,
    name,
    publication,
  }))
}
