//! Command editor operations, shared validation and the changes a person confirms in place.
use std::path::Path;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, Tool, ToolAction, VersionNumber},
  store::{ResourceStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  data::{ProjectTools, ToolEntry, create_tool},
  program::fixed_program,
  widening::changes,
};
use crate::{
  connections::mcp::{Allowed, program_problem_message},
  drafts,
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

fn apply(db: &mut ProjectDb, root: &Path, edit: Edit, allowed: &Allowed) -> Result<ResourceId, EditorError> {
  let expected = match &edit {
    Edit::Save { before, .. } => before.as_ref(),
    Edit::Publish(e) | Edit::KeepDraft(e) | Edit::StartLatest(e) => Some(e),
  };
  let listed = ProjectTools::load(db)?;
  if let Some(expected) = expected {
    if listed.tool(expected.id) != Some(expected) {
      return Err(StoreError::DraftChanged(expected.id).into());
    }
    if !matches!(expected.tool.action(), ToolAction::Command(_)) {
      return Err(EditorError::Message(
        Message::new("appMcp.projectTools.errors.notACommandTool").with("tool", expected.name.to_string()),
      ));
    }
  }
  let latest = expected.map(|e| db.latest_version::<Tool>(e.id)).transpose()?.flatten();
  if !allowed.covers(&edit.reasons(latest.as_ref().map(|v| v.body()))) {
    return Err(EditorError::Message(Message::new("errors.draftChanged")));
  }
  let now = Timestamp::now();
  match edit {
    Edit::Save { name, tool, before } => {
      let tool = fixed_tool(db, root, &tool)?;
      match before {
        Some(e) => {
          drafts::save_draft(db, e.id, tool, now)?;
          Ok(e.id)
        }
        None => {
          new_name(&listed, name.as_str()).map_err(EditorError::Message)?;
          Ok(create_tool(db, &name, &tool, now)?)
        }
      }
    }
    Edit::Publish(e) => {
      db.publish_draft_matching(e.id, &e.tool, now)?;
      Ok(e.id)
    }
    Edit::KeepDraft(e) => {
      db.save_draft(e.id, &Draft::new(e.tool, latest.as_ref().map(|v| v.number()), now))?;
      Ok(e.id)
    }
    Edit::StartLatest(e) => {
      let latest = latest.ok_or(StoreError::NoDraft(e.id))?;
      db.save_draft(e.id, &Draft::new(latest.body().clone(), Some(latest.number()), now))?;
      Ok(e.id)
    }
  }
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
