//! Skills editor rules shared by the apps and Orchester's MCP tools (`save_skill_draft`, `publish_skill`): a name of
//! the skill's own, a scope with an agent that loads it, at most [`MAX_FILES`] reference files of at most
//! [`MAX_FILE_BYTES`] each at paths inside the skill's folder, and Publish only of a reviewed, current draft. All disk
//! and database work is blocking.
use std::{fs, path::Path, sync::Arc};

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  id::ResourceId,
  name::Name,
  resource::{
    Draft, InstructionScope, Pipeline, ResourceBody, ResourceKind, Skill, SkillFile, SkillFilePath, Slug, StepKind,
    VersionNumber,
  },
  store::{ResourceStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  data::{ProjectSkills, SkillEntry},
  repository::{self, RepositoryScan},
};
use crate::{
  messages::{self, Message},
  project_settings,
};

/// The most reference files a skill has.
pub const MAX_FILES: usize = 100;
/// The most bytes of one reference file.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Which of the editor's rules refused a write, as automation reads it ([`Self::code`]); its [`Message`] says it to
/// people.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
  InvalidName,
  NameTaken,
  ScopeUnavailable,
  InvalidFiles,
  NoDraft,
  StaleDraft,
  BlankDescription,
  BlankBody,
}

impl Refusal {
  /// The refusal's code on Orchester's MCP server.
  pub const fn code(self) -> &'static str {
    match self {
      Self::InvalidName => "invalid_name",
      Self::NameTaken => "name_taken",
      Self::ScopeUnavailable => "scope_unavailable",
      Self::InvalidFiles => "invalid_files",
      Self::NoDraft => "no_draft",
      Self::StaleDraft => "stale_draft",
      Self::BlankDescription => "blank_description",
      Self::BlankBody => "blank_body",
    }
  }
}

/// Why a checked skill write did not happen.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
  #[error("refused by the skill editor's rule {0:?}")]
  Refused(Refusal, Message),
  /// What the write is checked against could not be read ([`EditorData::stored_in`]).
  #[error("the project's skills could not be read")]
  Unread(Message),
  #[error(transparent)]
  Store(#[from] StoreError),
}

impl WriteError {
  pub fn message(&self) -> Message {
    match self {
      Self::Refused(_, message) | Self::Unread(message) => message.clone(),
      Self::Store(error) => messages::store_error(error),
    }
  }
}

impl From<(Refusal, Message)> for WriteError {
  fn from((refusal, message): (Refusal, Message)) -> Self {
    Self::Refused(refusal, message)
  }
}

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
  /// What the editor checks a write against: the skills and the scopes they can choose, without the repository's
  /// agent files and the runs that use them.
  pub fn stored_in(root: &Path) -> Result<Self, Message> {
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
    self.checked_name(id, text).map_err(|(_, message)| message)
  }
  /// [`Self::validate_name`], with the rule that refused it.
  fn checked_name(&self, id: Option<ResourceId>, text: &str) -> Result<Name, (Refusal, Message)> {
    let name = Name::new(text).map_err(|_| (Refusal::InvalidName, Message::new("skills.editorErrors.nameRequired")))?;
    if self
      .listed
      .entries
      .iter()
      .any(|e| Some(e.id) != id && e.name.eq_ignore_ascii_case(name.as_str()))
    {
      return Err((
        Refusal::NameTaken,
        Message::new("skills.editorErrors.nameTaken").with("name", name.as_str().to_owned()),
      ));
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
  fn checked_scope(&self, scope: &InstructionScope) -> Result<(), (Refusal, Message)> {
    self
      .validate_scope(scope)
      .map_err(|message| (Refusal::ScopeUnavailable, message))
  }
}
pub fn publish_reason(entry: &SkillEntry) -> Option<Message> {
  publish_refusal(entry).map(|(_, message)| message)
}
/// [`publish_reason`], with the rule that refuses it.
fn publish_refusal(entry: &SkillEntry) -> Option<(Refusal, Message)> {
  if !entry.state.has_draft() {
    return Some((Refusal::NoDraft, Message::new("skills.editorErrors.noDraft")));
  }
  if matches!(entry.state, crate::agents::data::VersionState::StaleDraft { .. }) {
    return Some((Refusal::StaleDraft, Message::new("skills.editorErrors.stale")));
  }
  entry.body.check_publishable().err().map(|_| {
    if entry.body.description().trim().is_empty() {
      (
        Refusal::BlankDescription,
        Message::new("skills.editorErrors.descriptionRequired"),
      )
    } else {
      (Refusal::BlankBody, Message::new("skills.editorErrors.bodyRequired"))
    }
  })
}
/// Save over exactly the entry the editor read; another writer's edits are never overwritten.
pub fn save_in(root: &Path, before: Option<&SkillEntry>, name: &str, body: &Skill) -> Result<SkillEntry, Message> {
  save_checked_in(root, (before, name, body), Timestamp::now()).map_err(|error| error.message())
}
/// [`save_in`] at `now`, the time the draft is saved at, with why it was refused.
pub fn save_checked_in(
  root: &Path,
  (before, name, body): (Option<&SkillEntry>, &str, &Skill),
  now: Timestamp,
) -> Result<SkillEntry, WriteError> {
  let data = EditorData::stored_in(root).map_err(WriteError::Unread)?;
  let name = data.checked_name(before.map(|e| e.id), name)?;
  // An existing obsolete scope can be preserved while editing its content.
  if before.is_none_or(|e| e.body.scope() != body.scope()) {
    data.checked_scope(body.scope())?;
  }
  checked_files(body.files(), before.map_or(&[], |e| e.body.files()))?;
  let mut db = ProjectDb::open(root)?;
  let id = if let Some(before) = before {
    db.save_draft_and_name_matching(
      before.id,
      &before.body,
      before.state.published(),
      &Draft::new(body.clone(), before.based_on(), now),
      Some((&before.name, &name)),
    )?;
    before.id
  } else {
    let taken = db.resources(ResourceKind::Skill)?;
    let taken = taken.iter().map(|e| e.slug().clone()).collect::<Vec<_>>();
    let fallback = Slug::new("skill").expect("skill is a valid slug");
    db.create_resource(Slug::unique_for(&name, &fallback, &taken), name, body, now)?
      .id()
  };
  stored(&db, id)
}
/// Publishes only the reviewed saved draft and published version, checked atomically against storage.
pub fn publish_in(root: &Path, entry: &SkillEntry) -> Result<SkillEntry, Message> {
  publish_checked_in(root, entry).map_err(|error| error.message())
}
/// [`publish_in`], with why it was refused.
pub fn publish_checked_in(root: &Path, entry: &SkillEntry) -> Result<SkillEntry, WriteError> {
  if let Some(refusal) = publish_refusal(entry) {
    return Err(refusal.into());
  }
  let data = EditorData::stored_in(root).map_err(WriteError::Unread)?;
  data.checked_scope(entry.body.scope())?;
  let mut db = ProjectDb::open(root)?;
  db.publish_draft_matching_version(entry.id, &entry.body, entry.state.published(), Timestamp::now())?;
  stored(&db, entry.id)
}
/// Skill `id` as stored now.
fn stored(db: &ProjectDb, id: ResourceId) -> Result<SkillEntry, WriteError> {
  ProjectSkills::load(db)?
    .entry(id)
    .cloned()
    .ok_or(WriteError::Store(StoreError::ResourceNotFound(id)))
}
/// Records the draft `saved` holds as automation's (ORC-79, as ORC-30 does for instructions) when it is the one saved
/// at `at`: a later save, or none kept, is not. The Skills page marks it until a person saves it again or it is
/// published.
pub fn mark_automation_in(root: &Path, saved: &SkillEntry, at: Timestamp) -> Result<(), WriteError> {
  if saved.draft_saved_at == Some(at) {
    ProjectDb::open(root)?.mark_automation_draft(saved.id, at)?;
  }
  Ok(())
}
pub fn set_agent_files_in(root: &Path, enabled: bool) -> Result<(), Message> {
  project_settings::save_agent_files_in(root, enabled).map_err(|e| messages::store_error(&e))
}
/// Editable skill content is validated here, including reference-file paths, duplicates and limits.
pub struct SkillForm<'a> {
  pub name: &'a str,
  pub description: &'a str,
  pub body: &'a str,
  pub files: Vec<SkillFile>,
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
    self.checked(data, id).map_err(|error| error.message())
  }
  /// [`Self::skill`], with the rule that refused it.
  pub fn checked(self, data: &EditorData, id: Option<ResourceId>) -> Result<Skill, WriteError> {
    data.checked_name(id, self.name)?;
    let before = id.and_then(|id| data.listed.entry(id));
    checked_files(&self.files, before.map_or(&[], |e| e.body.files()))?;
    Skill::new(
      self.description.to_owned(),
      self.body.to_owned(),
      self.files,
      self.scope,
    )
    .map_err(|_| invalid_files(Message::new("skills.editorErrors.duplicateFile")))
  }
}
/// Refused past [`MAX_FILES`] files or with a file past [`MAX_FILE_BYTES`]; what the skill had (`before`, imported
/// from a repository with more or larger files) it keeps.
fn checked_files(files: &[SkillFile], before: &[SkillFile]) -> Result<(), WriteError> {
  if files.len() > MAX_FILES.max(before.len()) {
    return Err(too_many_files());
  }
  match files
    .iter()
    .find(|file| file.size > MAX_FILE_BYTES && !before.contains(file))
  {
    Some(file) => Err(file_too_large(file.path.as_str())),
    None => Ok(()),
  }
}
/// A reference file as automation writes it: its path in the skill's folder and its text, or no text to keep the
/// skill's file at that path as it is.
pub struct FileText<'a> {
  pub path: &'a str,
  pub content: Option<&'a str>,
}
/// What a reference file of [`files_in`] holds.
enum Content<'a> {
  Text(&'a str),
  Kept(SkillFile),
}
/// The reference files `given`, each new text stored among the project's blobs and each without text the file of
/// `kept` at its path; refused, before anything is stored, past the editor's limits, for a path outside the skill's
/// folder (absolute, with `.` or `..`, a drive or characters Windows does not allow, or `SKILL.md`), two files at one
/// path, or a kept file the skill does not have.
pub fn files_in(root: &Path, given: &[FileText<'_>], kept: &[SkillFile]) -> Result<Vec<SkillFile>, WriteError> {
  if given.len() > MAX_FILES.max(kept.len()) {
    return Err(too_many_files());
  }
  let mut checked: Vec<(SkillFilePath, Content<'_>)> = Vec::with_capacity(given.len());
  for file in given {
    let path = SkillFilePath::new(file.path)
      .map_err(|_| invalid_files(Message::new("skills.editorErrors.filePath").with("path", file.path.to_owned())))?;
    if checked.iter().any(|(taken, _)| *taken == path) {
      return Err(invalid_files(Message::new("skills.editorErrors.duplicateFile")));
    }
    let content = match file.content {
      Some(text) if text.len() as u64 > MAX_FILE_BYTES => return Err(file_too_large(file.path)),
      Some(text) => Content::Text(text),
      None => Content::Kept(kept.iter().find(|kept| kept.path == path).cloned().ok_or_else(|| {
        invalid_files(Message::new("skills.editorErrors.fileMissing").with("path", file.path.to_owned()))
      })?),
    };
    checked.push((path, content));
  }
  let mut db = ProjectDb::open(root)?;
  checked
    .into_iter()
    .map(|(path, content)| match content {
      Content::Text(text) => Ok(SkillFile {
        content: super::data::put_file(&mut db, text.as_bytes())?,
        size: text.len() as u64,
        path,
      }),
      Content::Kept(file) => Ok(file),
    })
    .collect()
}
/// Adds a reference file only when its path is free in the skill, within the editor's limits.
pub fn add_file_in(root: &Path, path: &Path, files: &[SkillFile]) -> Result<SkillFile, Message> {
  if files.len() >= MAX_FILES {
    return Err(too_many_files().message());
  }
  let size = fs::metadata(path)
    .map_err(|_| Message::new("skills.editorErrors.fileRead"))?
    .len();
  if size > MAX_FILE_BYTES {
    let name = path
      .file_name()
      .map(|name| name.to_string_lossy().into_owned())
      .unwrap_or_default();
    return Err(file_too_large(&name).message());
  }
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
fn invalid_files(message: Message) -> WriteError {
  WriteError::Refused(Refusal::InvalidFiles, message)
}
fn too_many_files() -> WriteError {
  invalid_files(Message::new("skills.editorErrors.tooManyFiles").with("max", MAX_FILES.to_string()))
}
fn file_too_large(path: &str) -> WriteError {
  invalid_files(
    Message::new("skills.editorErrors.fileTooLarge")
      .with("path", path.to_owned())
      .with("max", (MAX_FILE_BYTES / 1024).to_string()),
  )
}
