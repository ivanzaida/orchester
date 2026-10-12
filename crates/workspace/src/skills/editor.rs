//! Skills editor rules shared by the apps. All disk and database work is blocking.
use std::{path::Path, sync::Arc};

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  id::ResourceId,
  name::Name,
  resource::{Draft, InstructionScope, Pipeline, ResourceBody, ResourceKind, Skill, Slug, StepKind, VersionNumber},
  store::{ResourceStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  data::{ProjectSkills, SkillEntry},
  repository::{self, RepositoryScan},
};
use crate::{
  messages::{self, Message},
  project_settings,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeOption {
  pub scope: InstructionScope,
  pub pipeline: Option<Arc<str>>,
  pub step: Option<Arc<str>>,
  pub available: bool,
  pub agent: Option<Arc<str>>,
  pub published: Option<VersionNumber>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorData {
  pub listed: ProjectSkills,
  pub repository: RepositoryScan,
  pub agent_files: RepositoryAgentFiles,
  pub scopes: Vec<ScopeOption>,
  pub active: Vec<super::uses::ActiveUse>,
}
impl EditorData {
  pub fn load_in(root: &Path) -> Result<Self, Message> {
    let mut data = Self::stored_in(root)?;
    data.active =
      super::uses::active(&ProjectDb::open(root).map_err(|e| messages::store_error(&e))?).map_err(|e| e.message())?;
    data.repository = repository::scan(root).map_err(|_| Message::new("skills.library.loadFailed"))?;
    Ok(data)
  }
  fn stored_in(root: &Path) -> Result<Self, Message> {
    let db = ProjectDb::open(root).map_err(|e| messages::store_error(&e))?;
    let listed = ProjectSkills::load(&db).map_err(|e| messages::store_error(&e))?;
    let mut scopes = vec![ScopeOption {
      scope: InstructionScope::Project,
      pipeline: None,
      step: None,
      available: true,
      agent: None,
      published: None,
    }];
    scopes.extend(listed.roles().into_iter().map(|role| ScopeOption {
      agent: Some(role.as_str().into()),
      published: None,
      available: true,
      scope: InstructionScope::AgentRole { role },
      pipeline: None,
      step: None,
    }));
    for resource in db
      .resources(ResourceKind::Pipeline)
      .map_err(|e| messages::store_error(&e))?
    {
      let draft = db
        .draft::<Pipeline>(resource.id())
        .map_err(|e| messages::store_error(&e))?;
      let latest = db
        .latest_version::<Pipeline>(resource.id())
        .map_err(|e| messages::store_error(&e))?;
      let Some(body) = draft
        .as_ref()
        .map(Draft::body)
        .or_else(|| latest.as_ref().map(|v| v.body()))
      else {
        continue;
      };
      let name: Arc<str> = resource.name().as_str().into();
      scopes.push(ScopeOption {
        scope: InstructionScope::Pipeline {
          pipeline: resource.id(),
        },
        pipeline: Some(name.clone()),
        step: None,
        available: true,
        agent: None,
        published: latest.as_ref().map(|version| version.number()),
      });
      scopes.extend(body.steps().iter().enumerate().map(|(index, step)| {
        ScopeOption {
          scope: InstructionScope::Step {
            pipeline: resource.id(),
            step: step.key.clone(),
          },
          pipeline: Some(name.clone()),
          step: Some(format!("{:02} {}", index + 1, step.name).into()),
          available: matches!(step.kind, StepKind::Agent { .. }),
          published: None,
          agent: match &step.kind {
            StepKind::Agent { profile: Some(id), .. } => listed
              .profiles
              .iter()
              .find(|p| p.id == *id)
              .map(|p| p.role.as_str().into()),
            _ => None,
          },
        }
      }));
    }
    Ok(Self {
      listed,
      scopes,
      active: Vec::new(),
      repository: RepositoryScan {
        items: Arc::new(Vec::new()),
      },
      agent_files: db.repository_agent_files().map_err(|e| messages::store_error(&e))?,
    })
  }
  pub fn validate_name(&self, id: Option<ResourceId>, text: &str) -> Result<Name, Message> {
    let name = Name::new(text).map_err(|_| Message::new("skills.editorErrors.nameRequired"))?;
    if self
      .listed
      .entries
      .iter()
      .any(|e| Some(e.id) != id && e.name.eq_ignore_ascii_case(name.as_str()))
    {
      return Err(Message::new("skills.editorErrors.nameTaken").with("name", name.as_str().to_owned()));
    }
    Ok(name)
  }
  pub fn validate_scope(&self, scope: &InstructionScope) -> Result<(), Message> {
    if self
      .scopes
      .iter()
      .any(|option| &option.scope == scope && option.available)
    {
      return Ok(());
    }
    Err(Message::new("skills.editorErrors.scopeUnavailable"))
  }
}
pub fn publish_reason(entry: &SkillEntry) -> Option<Message> {
  if !entry.state.has_draft() {
    return Some(Message::new("skills.editorErrors.noDraft"));
  }
  if matches!(entry.state, crate::agents::data::VersionState::StaleDraft { .. }) {
    return Some(Message::new("skills.editorErrors.stale"));
  }
  entry.body.check_publishable().err().map(|_| {
    Message::new(if entry.body.description().trim().is_empty() {
      "skills.editorErrors.descriptionRequired"
    } else {
      "skills.editorErrors.bodyRequired"
    })
  })
}
/// Save over exactly the entry the editor read; another writer's edits are never overwritten.
pub fn save_in(root: &Path, before: Option<&SkillEntry>, name: &str, body: &Skill) -> Result<SkillEntry, Message> {
  let data = EditorData::stored_in(root)?;
  let name = data.validate_name(before.map(|e| e.id), name)?;
  // An existing obsolete scope can be preserved while editing its content.
  if before.is_none_or(|e| e.body.scope() != body.scope()) {
    data.validate_scope(body.scope())?;
  }
  let mut db = ProjectDb::open(root).map_err(|e| messages::store_error(&e))?;
  let id = if let Some(before) = before {
    db.save_draft_and_name_matching(
      before.id,
      &before.body,
      before.state.published(),
      &Draft::new(body.clone(), before.based_on(), Timestamp::now()),
      Some((&before.name, &name)),
    )
    .map_err(|e| messages::store_error(&e))?;
    before.id
  } else {
    let taken = db
      .resources(ResourceKind::Skill)
      .map_err(|e| messages::store_error(&e))?;
    let taken = taken.iter().map(|e| e.slug().clone()).collect::<Vec<_>>();
    let fallback = Slug::new("skill").expect("skill is a valid slug");
    db.create_resource(Slug::unique_for(&name, &fallback, &taken), name, body, Timestamp::now())
      .map_err(|e| messages::store_error(&e))?
      .id()
  };
  ProjectSkills::load(&db)
    .map_err(|e| messages::store_error(&e))?
    .entry(id)
    .cloned()
    .ok_or_else(|| Message::new("skills.library.loadFailed"))
}
/// Publishes only the reviewed saved draft and published version, checked atomically against storage.
pub fn publish_in(root: &Path, entry: &SkillEntry) -> Result<SkillEntry, Message> {
  if let Some(reason) = publish_reason(entry) {
    return Err(reason);
  }
  let data = EditorData::stored_in(root)?;
  data.validate_scope(entry.body.scope())?;
  let mut db = ProjectDb::open(root).map_err(|e| messages::store_error(&e))?;
  db.publish_draft_matching_version(entry.id, &entry.body, entry.state.published(), Timestamp::now())
    .map_err(|e| messages::store_error(&e))?;
  ProjectSkills::load(&db)
    .map_err(|e| messages::store_error(&e))?
    .entry(entry.id)
    .cloned()
    .ok_or_else(|| Message::new("skills.library.loadFailed"))
}
pub fn set_agent_files_in(root: &Path, enabled: bool) -> Result<(), Message> {
  project_settings::save_agent_files_in(root, enabled).map_err(|e| messages::store_error(&e))
}
/// Editable skill content is validated here, including reference-file paths and duplicates.
pub struct SkillForm<'a> {
  pub name: &'a str,
  pub description: &'a str,
  pub body: &'a str,
  pub files: Vec<orchester_core::resource::SkillFile>,
  pub scope: InstructionScope,
}
impl SkillForm<'_> {
  /// Compare all raw fields before validation, including a new skill's files and scope.
  pub fn has_edits(&self, before: Option<&SkillEntry>) -> bool {
    if let Some(before) = before {
      self.name != before.name.as_ref()
        || self.description != before.body.description()
        || self.body != before.body.body()
        || self.files != before.body.files()
        || &self.scope != before.body.scope()
    } else {
      !self.name.is_empty()
        || !self.description.is_empty()
        || !self.body.is_empty()
        || !self.files.is_empty()
        || self.scope != InstructionScope::Project
    }
  }
  pub fn skill(self, data: &EditorData, id: Option<ResourceId>) -> Result<Skill, Message> {
    data.validate_name(id, self.name)?;
    Skill::new(
      self.description.to_owned(),
      self.body.to_owned(),
      self.files,
      self.scope,
    )
    .map_err(|_| Message::new("skills.editorErrors.duplicateFile"))
  }
}
/// Adds a reference file only when its path is free in the skill.
pub fn add_file_in(
  root: &Path,
  path: &Path,
  files: &[orchester_core::resource::SkillFile],
) -> Result<orchester_core::resource::SkillFile, Message> {
  let added = super::data::add_file_in(root, path).map_err(|error| match error {
    super::data::AddFileError::Store(e) => messages::store_error(&e),
    super::data::AddFileError::Read(..) | super::data::AddFileError::Name(_) => {
      Message::new("skills.editorErrors.fileRead")
    }
  })?;
  if files.iter().any(|file| file.path == added.path) {
    return Err(Message::new("skills.editorErrors.duplicateFile"));
  }
  Ok(added)
}
