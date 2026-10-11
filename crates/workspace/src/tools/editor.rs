//! Command editor operations, shared validation and the changes a person confirms in place.
use std::path::Path;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, Tool, ToolAction, VersionNumber},
  store::{ResourceStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, ResourceUpdate, StoreError};

use super::{
  data::{ProjectTools, ToolEntry, create_tool},
  program::fixed_program,
  widening::changes,
};
use crate::{
  connections::mcp::{Allowed, program_problem_message},
  messages::{self, Message},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorData {
  pub listed: ProjectTools,
  pub latest: Option<Tool>,
}

impl EditorData {
  pub fn load_in(root: &Path, id: Option<ResourceId>) -> Result<Self, StoreError> {
    let db = ProjectDb::open(root)?;
    let listed = ProjectTools::load(&db)?;
    let latest = id
      .filter(|id| listed.tool(*id).is_some())
      .map(|id| db.latest_version::<Tool>(id))
      .transpose()?
      .flatten()
      .map(|v| v.body().clone());
    Ok(Self { listed, latest })
  }
}

/// An operation on exactly the tool the editor read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
  Save {
    name: Name,
    tool: Box<Tool>,
    before: Option<ToolEntry>,
  },
  Publish(ToolEntry),
  KeepDraft(ToolEntry),
  StartLatest(ToolEntry),
}

impl Edit {
  pub fn reasons(&self, latest: Option<&Tool>) -> Vec<Message> {
    match self {
      Self::Save { tool, before, .. } => changes(
        before.as_ref().map(|e| &e.tool),
        tool,
        "appMcp.projectTools.widens.newTool",
      ),
      Self::Publish(entry) => changes(latest, &entry.tool, "appMcp.projectTools.widens.firstVersion"),
      Self::KeepDraft(_) | Self::StartLatest(_) => Vec::new(),
    }
  }
}

/// Checks that a new name is valid and not already taken, ignoring case.
pub fn new_name(listed: &ProjectTools, text: &str) -> Result<Name, Message> {
  let name = super::form::tool_name(text)?;
  if listed
    .tools
    .iter()
    .any(|e| e.name.to_lowercase() == name.as_str().to_lowercase())
  {
    return Err(Message::new("appMcp.projectTools.errors.nameTaken").with("name", name.as_str().to_owned()));
  }
  Ok(name)
}

/// Next publish number, including the version limit.
pub fn next_version(entry: Option<&ToolEntry>) -> Option<VersionNumber> {
  match entry.and_then(|e| e.state.published()) {
    Some(v) => v.next(),
    None => Some(VersionNumber::FIRST),
  }
}

/// Stores only the operation and widenings the person reviewed. Programs are fixed outside the repository.
pub fn apply_in(root: &Path, edit: Edit, allowed: &Allowed) -> Result<ResourceId, Message> {
  let mut db = ProjectDb::open(root).map_err(|e| messages::store_error(&e))?;
  apply(&mut db, root, edit, allowed).map_err(|e| match e {
    EditorError::Store(e) => messages::store_error(&e),
    EditorError::Message(m) => m,
  })
}

/// Runs the real editor operation on a fixture connection whose lock waits can be observed.
#[cfg(feature = "test-support")]
pub fn apply_with_db_for_test(
  db: &mut ProjectDb,
  root: &Path,
  edit: Edit,
  allowed: &Allowed,
) -> Result<ResourceId, Message> {
  apply(db, root, edit, allowed).map_err(|e| match e {
    EditorError::Store(e) => messages::store_error(&e),
    EditorError::Message(m) => m,
  })
}

/// Resolves and checks the program for a reviewable draft, without storing anything.
/// Call off the UI thread; applying the edit checks the file again.
pub fn prepare_in(root: &Path, tool: &Tool) -> Result<Tool, Message> {
  let db = ProjectDb::open(root).map_err(|e| messages::store_error(&e))?;
  fixed_tool(&db, root, tool).map_err(|e| match e {
    EditorError::Store(e) => messages::store_error(&e),
    EditorError::Message(m) => m,
  })
}

enum EditorError {
  Store(StoreError),
  Message(Message),
}
impl From<StoreError> for EditorError {
  fn from(e: StoreError) -> Self {
    Self::Store(e)
  }
}

fn apply(db: &mut ProjectDb, root: &Path, mut edit: Edit, allowed: &Allowed) -> Result<ResourceId, EditorError> {
  if let Edit::Save { tool, .. } = &mut edit {
    **tool = fixed_tool(db, root, tool)?;
  }
  if let Edit::Save {
    name,
    tool,
    before: None,
  } = &edit
  {
    if !allowed.covers(&edit.reasons(None)) {
      return Err(EditorError::Message(Message::new("errors.draftChanged")));
    }
    let listed = ProjectTools::load(db)?;
    new_name(&listed, name.as_str()).map_err(EditorError::Message)?;
    return Ok(create_tool(db, name, tool, Timestamp::now())?);
  }
  apply_existing(db, &edit, allowed)
}

fn apply_existing(db: &mut ProjectDb, edit: &Edit, allowed: &Allowed) -> Result<ResourceId, EditorError> {
  let expected = match edit {
    Edit::Save { before: Some(e), .. } | Edit::Publish(e) | Edit::KeepDraft(e) | Edit::StartLatest(e) => e,
    Edit::Save { before: None, .. } => unreachable!("new tools are created separately"),
  };
  if !matches!(expected.tool.action(), ToolAction::Command(_)) {
    return Err(EditorError::Message(
      Message::new("appMcp.projectTools.errors.notACommandTool").with("tool", expected.name.to_string()),
    ));
  }
  let id = expected.id;
  db.update_resource::<Tool, EditorError>(id, |resource, draft: Option<Draft<Tool>>, latest| {
    let based_on = draft
      .as_ref()
      .map_or_else(|| latest.as_ref().map(|v| v.number()), Draft::based_on);
    if ToolEntry::new(id, resource.name(), draft, latest.as_ref()).as_ref() != Some(expected) {
      return Err(StoreError::DraftChanged(id).into());
    }
    if !allowed.covers(&edit.reasons(latest.as_ref().map(|v| v.body()))) {
      return Err(StoreError::DraftChanged(id).into());
    }
    let now = Timestamp::now();
    match edit {
      Edit::Save { tool, .. } => Ok(ResourceUpdate::SaveDraft(Draft::new(*tool.clone(), based_on, now))),
      Edit::Publish(_) => Ok(ResourceUpdate::PublishDraft(now)),
      Edit::KeepDraft(e) => Ok(ResourceUpdate::SaveDraft(Draft::new(
        e.tool.clone(),
        latest.as_ref().map(|v| v.number()),
        now,
      ))),
      Edit::StartLatest(_) => {
        let latest = latest.ok_or(StoreError::NoDraft(id))?;
        Ok(ResourceUpdate::SaveDraft(Draft::new(
          latest.body().clone(),
          Some(latest.number()),
          now,
        )))
      }
    }
  })?;
  Ok(id)
}

fn fixed_tool(db: &ProjectDb, root: &Path, tool: &Tool) -> Result<Tool, EditorError> {
  let ToolAction::Command(command) = tool.action() else {
    return Err(EditorError::Message(Message::new("tools.errors.programRequired")));
  };
  let project = db.project_environment()?;
  let path = command.search_path(&project, orchester_exec::search_path::search_path());
  let program = fixed_program(command.program(), path.as_deref(), root)
    .map_err(|p| EditorError::Message(program_problem_message(&p)))?;
  let action = orchester_core::resource::CommandAction::new(program, command.args().to_vec())
    .with_timeout(command.timeout())
    .with_exit_codes(command.exit_codes().clone())
    .with_output_limit(command.output_limit())
    .with_environment(command.environment().clone());
  Ok(
    Tool::new(tool.description(), ToolAction::Command(action), tool.access().cloned())
      .expect("a command tool needs no access policy"),
  )
}
