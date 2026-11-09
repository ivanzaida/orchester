//! Editing the fields of a local profile in its panel (`Agents · Profile` L1, `docs/design/team-configuration.md`
//! rule 11): the Agent CLI, the model, the reasoning effort and the MCP servers it chooses are changed one at a time,
//! each a save of the profile's draft as the editor's autosave makes it, and *Publish vN* publishes that draft. The
//! rules are the editor's ([`super::editor::apply`]): a change that widens what its agents may do asks first, which
//! only a person answers, and the profile is rebuilt by the editor's own form ([`ProfileForm`]), so nothing a field
//! cannot hold is saved.

use std::path::Path;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, invocation::Platform, mcp::McpServerSelection},
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, ModelId},
};

use super::{
  data::{ProfileEntry, ProjectAgents, VersionState},
  editor::{self, Action, Outcome, Request},
  form::{
    ModelSelection, ProfileErrors, ProfileForm, access_for_connection, effort_options, first_model, listed_models,
    stored_effort,
  },
};
use crate::messages::{self, Message};

/// One field of a local profile, changed to a new value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
  /// Runs with another CLI: on the first connection of it the project has, on that connection's first model, at the
  /// CLI's default effort, with the access the new CLI can keep.
  Cli(AgentProvider),
  /// Another model of its connection, at the CLI's default effort.
  Model(ModelId),
  /// Another reasoning effort of its model; `None` leaves it to the CLI.
  Effort(Option<String>),
  /// Chooses an MCP server of the project as well.
  AddServer(ConnectionId),
  /// No longer chooses the MCP server.
  RemoveServer(ConnectionId),
}

/// Saves `edit` of the local profile `profile` as its draft. `allowed` is what the person already allowed to widen:
/// [`Outcome::Confirm`] lists what `edit` widens beyond it and saves nothing. A change that leaves the profile as it
/// is saves nothing and says [`Outcome::Saved`].
pub fn edit(root: &Path, profile: ResourceId, edit: &Edit, allowed: &[Message]) -> Result<Outcome, Message> {
  let listed = ProjectAgents::load_in(root).map_err(|error| messages::store_error(&error))?;
  let entry = listed
    .profile(profile)
    .cloned()
    .ok_or_else(|| Message::new("agents.localConnection.gone"))?;
  let next = changed(&listed, &entry.profile, edit)?;
  if next == entry.profile {
    return Ok(Outcome::Saved(profile));
  }
  apply(root, entry, next, Action::Save, allowed)
}

/// Publishes the draft of the local profile `profile` as its next version, with what the person allowed to widen. A
/// draft that was started from an older version than the latest is not published here: its editor compares the two.
/// `decided`: a person pressed Publish (or automation they let decide), which what publishing changes in the role's
/// prompts needs.
pub fn publish(root: &Path, profile: ResourceId, allowed: &[Message], decided: bool) -> Result<Outcome, Message> {
  let entry = editor::entry_in(root, profile)?;
  match entry.state {
    VersionState::Draft { .. } => {}
    VersionState::StaleDraft { .. } => return Err(Message::new("agents.localEdit.staleDraft")),
    VersionState::Published(_) => return Err(Message::new("appMcp.errors.noDraft")),
  }
  let body = entry.profile.clone();
  let request = request(root, entry, body, Action::Publish, allowed)?;
  editor::apply(
    root,
    &Request {
      prompt_decided: decided,
      ..request
    },
  )
}

fn apply(
  root: &Path,
  entry: ProfileEntry,
  body: AgentProfile,
  action: Action,
  allowed: &[Message],
) -> Result<Outcome, Message> {
  editor::apply(root, &request(root, entry, body, action, allowed)?)
}

/// The request for `action` with `body`, which takes as allowed what the stored draft already widens, as the editor's
/// autosave does.
fn request(
  root: &Path,
  entry: ProfileEntry,
  body: AgentProfile,
  action: Action,
  allowed: &[Message],
) -> Result<Request, Message> {
  let mut allowed = allowed.to_vec();
  for reason in editor::approved_in(root, &entry)? {
    if !allowed.contains(&reason) {
      allowed.push(reason);
    }
  }
  Ok(Request {
    existing: Some(entry),
    profile: body,
    action,
    allowed,
    prompt_decided: false,
  })
}

/// `current` with `edit` made, by the editor's own form.
fn changed(listed: &ProjectAgents, current: &AgentProfile, edit: &Edit) -> Result<AgentProfile, Message> {
  let connection = listed
    .connections
    .iter()
    .find(|found| found.id() == current.connection());
  let mut fields = Fields {
    connection,
    model: current.model().as_str().to_owned(),
    effort: stored_effort(current),
    permissions: current.permissions().clone(),
    servers: current.mcp_servers().clone(),
  };
  match edit {
    Edit::Cli(provider) => fields.use_cli(listed, *provider)?,
    Edit::Model(model) => fields.use_model(model)?,
    Edit::Effort(effort) => fields.use_effort(effort.as_deref())?,
    Edit::AddServer(server) => {
      let mut ids = fields.servers.ids().to_vec();
      if !ids.contains(server) {
        ids.push(*server);
      }
      fields.servers = selection(ids)?;
    }
    Edit::RemoveServer(server) => {
      let ids = fields.servers.ids().iter().copied().filter(|id| id != server).collect();
      fields.servers = selection(ids)?;
    }
  }
  ProfileForm {
    role: current.role().as_str(),
    responsibilities: current.responsibilities(),
    instructions: current.instructions().as_str(),
    connection: fields.connection,
    model: &ModelSelection::Custom,
    custom_model: &fields.model,
    effort: fields.effort.as_deref(),
    permissions: &fields.permissions,
    mcp_servers: &fields.servers,
  }
  .profile()
  .map_err(|errors| first_error(&errors))
}

fn selection(ids: Vec<ConnectionId>) -> Result<McpServerSelection, Message> {
  McpServerSelection::new(ids).map_err(|_| Message::new("agents.localEdit.serversInvalid"))
}

/// The editor's fields of the profile as they are while one is changed.
struct Fields<'a> {
  connection: Option<&'a AgentConnection>,
  model: String,
  effort: Option<String>,
  permissions: orchester_core::permission::AgentPermissions,
  servers: McpServerSelection,
}

impl<'a> Fields<'a> {
  fn use_cli(&mut self, listed: &'a ProjectAgents, provider: AgentProvider) -> Result<(), Message> {
    let chosen = listed
      .connections
      .iter()
      .find(|found| found.provider() == provider)
      .ok_or_else(|| Message::new("agents.localEdit.noConnection"))?;
    let model = first_model(chosen).ok_or_else(|| Message::new("errors.modelRequired"))?;
    let (permissions, servers) = access_for_connection(&self.permissions, &self.servers, provider, Platform::current());
    *self = Self {
      connection: Some(chosen),
      model: model.as_str().to_owned(),
      effort: None,
      permissions,
      servers,
    };
    Ok(())
  }

  fn use_model(&mut self, model: &ModelId) -> Result<(), Message> {
    let connection = self
      .connection
      .ok_or_else(|| Message::new("agents.localEdit.noConnection"))?;
    if model.as_str() != self.model && !listed_models(connection).contains(model) {
      return Err(Message::new("agents.localEdit.modelNotOffered"));
    }
    model.as_str().clone_into(&mut self.model);
    self.effort = None;
    Ok(())
  }

  fn use_effort(&mut self, effort: Option<&str>) -> Result<(), Message> {
    let connection = self
      .connection
      .ok_or_else(|| Message::new("agents.localEdit.noConnection"))?;
    if let Some(effort) = effort {
      let model = ModelId::new(&self.model).map_err(|_| Message::new("errors.modelInvalid"))?;
      // A provider's models are not known: the form checks the level its CLI takes.
      if connection.api_provider().is_none()
        && !effort_options(connection, &model)
          .iter()
          .any(|offered| offered == effort)
      {
        return Err(Message::new("agents.localEdit.effortNotOffered"));
      }
    }
    self.effort = effort.map(str::to_owned);
    Ok(())
  }
}

/// The first thing the form says is wrong, in the form's field order.
fn first_error(errors: &ProfileErrors) -> Message {
  [
    &errors.role,
    &errors.instructions,
    &errors.connection,
    &errors.model,
    &errors.effort,
  ]
  .into_iter()
  .flatten()
  .next()
  .cloned()
  .unwrap_or_else(|| Message::new("errors.draftChanged"))
}
