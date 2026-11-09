//! Editing the fields of a local profile in its panel (`Agents · Profile` L1, `docs/design/team-configuration.md`
//! rule 11): the Agent CLI, the model, the reasoning effort and the MCP servers it chooses are changed one at a time,
//! each a save of the profile's draft as the editor's autosave makes it, and *Publish vN* publishes that draft. The
//! rules are the editor's ([`super::editor::apply`]): a change that widens what its agents may do asks first, which
//! only a person answers, and the profile is rebuilt by the editor's own form ([`ProfileForm`]), so nothing a field
//! cannot hold is saved.
//!
//! The Agents hub's panel holds its changes instead, and stores nothing until Save ([`hold`], [`save`]): each choice is
//! made by the same rules on what the panel holds, and Save publishes it as the next version at once.

use std::path::Path;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, invocation::Platform, mcp::McpServerSelection},
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, ModelId, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  data::{ProfileEntry, ProjectAgents, VersionState},
  editor::{self, Action, Outcome, Request},
  form::{
    ModelSelection, ProfileErrors, ProfileForm, access_for_connection, effort_options, first_model, listed_models,
    stored_effort,
  },
  local_connection::discard_saved,
  local_hub::{LocalProfile, profile_as},
};
use crate::{
  messages::{self, Message},
  prompt_changes, removal,
};

/// One field of a local profile, changed to a new value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
  /// Runs with another CLI: on the first connection of it the project has, on that connection's first model, at the
  /// CLI's default effort, with the access the new CLI can keep.
  Cli(AgentProvider),
  /// Another model of its connection, by its id, at the CLI's default effort.
  Model(String),
  /// Another reasoning effort of its model; `None` leaves it to the CLI.
  Effort(Option<String>),
  /// Chooses an MCP server of the project as well.
  AddServer(ConnectionId),
  /// No longer chooses the MCP server.
  RemoveServer(ConnectionId),
  /// Runs on another connection of its CLI, with the same model and effort.
  Connection(ConnectionId),
}

/// What the panel of a local profile holds: changes that are not stored, and how the panel shows them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
  pub body: AgentProfile,
  pub shown: LocalProfile,
  /// Whether it differs from the latest published version: Save has something to publish.
  pub changed: bool,
}

/// `edit` made to what the panel of the local profile `profile` holds: `held`, or what is stored (its draft, else its
/// latest version) while it holds nothing yet. Nothing is stored.
pub fn hold(root: &Path, profile: ResourceId, held: Option<&AgentProfile>, edit: &Edit) -> Result<Held, Message> {
  let listed = ProjectAgents::load_in(root).map_err(|error| messages::store_error(&error))?;
  let entry = listed
    .profile(profile)
    .ok_or_else(|| Message::new("agents.localConnection.gone"))?;
  let body = changed(&listed, held.unwrap_or(&entry.profile), edit)?;
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let shown = profile_as(&db, profile, &body)
    .map_err(|error| messages::store_error(&error))?
    .ok_or_else(|| Message::new("agents.localConnection.gone"))?;
  let published = db
    .latest_version::<AgentProfile>(profile)
    .map_err(|error| messages::store_error(&error))?;
  Ok(Held {
    changed: published.as_ref().map(|version| version.body()) != Some(&body),
    body,
    shown,
  })
}

/// Publishes what the panel of the local profile `profile` holds as its next version at once: `held`, or its stored
/// draft while the panel holds nothing else. `allowed` is what the person already allowed to widen:
/// [`Outcome::Confirm`] lists what it widens beyond that, and nothing is stored. All or nothing: the draft the save
/// makes is thrown away when the publication is refused. Refused, as the panel's *Publish* was, for a stored draft
/// started from an older version, and for one that changes what the role's prompts say, which the panel does not show.
pub fn save(
  root: &Path,
  profile: ResourceId,
  held: Option<&AgentProfile>,
  allowed: &[Message],
) -> Result<Outcome, Message> {
  let listed = ProjectAgents::load_in(root).map_err(|error| messages::store_error(&error))?;
  let entry = listed
    .profile(profile)
    .cloned()
    .ok_or_else(|| Message::new("agents.localConnection.gone"))?;
  if matches!(entry.state, VersionState::StaleDraft { .. }) {
    return Err(Message::new("agents.localEdit.staleDraft"));
  }
  let body = held.cloned().unwrap_or_else(|| entry.profile.clone());
  let had_draft = entry.state.has_draft();
  if !had_draft && body == entry.profile {
    return Ok(Outcome::Saved(profile));
  }
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  if !prompt_changes::profile_in(&db, profile, &body)
    .map_err(|error| messages::store_error(&error))?
    .is_empty()
  {
    return Err(Message::new("agents.localEdit.promptsInEditor"));
  }
  if body != entry.profile
    && let Outcome::Confirm(reasons) = apply(root, entry, body.clone(), Action::Save, allowed)?
  {
    return Ok(Outcome::Confirm(reasons));
  }
  let drafted = editor::entry_in(root, profile)?;
  let published = apply(root, drafted, body.clone(), Action::Publish, allowed);
  if !had_draft && !matches!(published, Ok(Outcome::Saved(_))) {
    discard_saved(root, profile, &body);
  }
  published
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

/// Publishes the draft of the local profile `profile` as its next version, with what the person allowed to widen, while
/// it is still the draft saved at `shown`, the one the panel showed: another draft, or none, is refused as the editor
/// refuses it ([`editor::apply`] compares what was shown), and nothing is published. A draft that was started from an
/// older version than the latest is not published here either: its editor compares the two. Nor is one that changes
/// what the role's prompts say (its role, responsibilities or instructions): the panel does not show those, so pressing
/// *Publish* there is not a decision about them, and the editor, where they are shown, publishes it.
pub fn publish(root: &Path, profile: ResourceId, shown: Timestamp, allowed: &[Message]) -> Result<Outcome, Message> {
  let (entry, saved) = stored(root, profile)?;
  match entry.state {
    VersionState::Draft { .. } => {}
    VersionState::StaleDraft { .. } => return Err(Message::new("agents.localEdit.staleDraft")),
    VersionState::Published(_) => return Err(Message::new("appMcp.errors.noDraft")),
  }
  still_shown(saved, shown)?;
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  if !prompt_changes::profile_in(&db, profile, &entry.profile)
    .map_err(|error| messages::store_error(&error))?
    .is_empty()
  {
    return Err(Message::new("agents.localEdit.promptsInEditor"));
  }
  let body = entry.profile.clone();
  editor::apply(root, &request(root, entry, body, Action::Publish, allowed)?)
}

/// Discards the draft of the local profile `profile` while it is still the one saved at `shown`: the latest published
/// version is the profile again, and its number comes back. Another draft is refused and nothing is lost.
pub fn discard(root: &Path, profile: ResourceId, shown: Timestamp) -> Result<VersionNumber, Message> {
  let (entry, saved) = stored(root, profile)?;
  still_shown(saved, shown)?;
  removal::discard_in::<AgentProfile>(root, profile, &entry.profile)
}

/// The stored profile and when its draft was saved, read together.
fn stored(root: &Path, profile: ResourceId) -> Result<(ProfileEntry, Option<Timestamp>), Message> {
  let listed = ProjectAgents::load_in(root).map_err(|error| messages::store_error(&error))?;
  let entry = listed
    .profile(profile)
    .cloned()
    .ok_or_else(|| Message::new("agents.localConnection.gone"))?;
  Ok((entry, listed.draft_times.get(&profile).copied()))
}

fn still_shown(saved: Option<Timestamp>, shown: Timestamp) -> Result<(), Message> {
  if saved == Some(shown) {
    Ok(())
  } else {
    Err(Message::new("errors.draftChanged"))
  }
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
    Edit::Connection(connection) => fields.use_connection(listed, *connection)?,
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

  /// Another connection of the CLI it runs with: its connection's, and while that is gone its model's.
  fn use_connection(&mut self, listed: &'a ProjectAgents, connection: ConnectionId) -> Result<(), Message> {
    let cli = self
      .connection
      .map_or_else(|| AgentProvider::cli_of_model(&self.model), |own| Some(own.provider()));
    let chosen = listed
      .connections
      .iter()
      .find(|found| found.id() == connection && Some(found.provider()) == cli)
      .ok_or_else(|| Message::new("agents.localConnection.notOfKind"))?;
    self.connection = Some(chosen);
    Ok(())
  }

  fn use_model(&mut self, model: &str) -> Result<(), Message> {
    let connection = self
      .connection
      .ok_or_else(|| Message::new("agents.localEdit.noConnection"))?;
    let model = ModelId::new(model).map_err(|_| Message::new("errors.modelInvalid"))?;
    if model.as_str() != self.model && !listed_models(connection).contains(&model) {
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
