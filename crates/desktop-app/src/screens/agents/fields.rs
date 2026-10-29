//! A profile page's field values and the profile they make (validated by `orchester_workspace::agents::form`).
use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};
use orchester_core::{
  connection::{AgentConnection, invocation::Platform, mcp::McpServerSelection},
  permission::AgentPermissions,
  resource::AgentProfile,
};
use orchester_workspace::agents::{
  data::{ProfileEntry, ProjectAgents},
  form::{ModelSelection, ProfileErrors, ProfileForm, access_for_connection, first_model, stored_effort},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permissions(pub AgentPermissions);
impl DevtoolsInspectable for Permissions {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Servers(pub McpServerSelection);
impl DevtoolsInspectable for Servers {}

#[derive(Clone)]
pub struct Fields {
  pub role: Signal<String>,
  pub responsibility: Signal<String>,
  pub instructions: Signal<String>,
  pub connection: Signal<String>,
  pub model: Signal<String>,
  pub custom: Signal<String>,
  pub effort: Signal<String>,
  pub model_open: Signal<bool>,
  pub connection_open: Signal<bool>,
  pub effort_open: Signal<bool>,
  pub permissions: Signal<Permissions>,
  pub servers: Signal<Servers>,
  /// The connection changed on this page: what it cannot enforce was reset, which the page says.
  pub changed_connection: Signal<bool>,
}

impl Fields {
  pub fn new(ctx: &mut Ctx, listed: &ProjectAgents, entry: Option<&ProfileEntry>) -> Self {
    let profile = entry.map(|e| &e.profile);
    let connection = profile
      .and_then(|p| listed.connections.iter().find(|c| c.id() == p.connection()))
      .or_else(|| {
        if profile.is_none() {
          listed.connections.first()
        } else {
          None
        }
      });
    let permissions = profile.map_or_else(
      || {
        connection.map_or_else(AgentPermissions::before_permissions, |c| {
          AgentPermissions::most_restrictive(c.provider(), Platform::current())
        })
      },
      |p| p.permissions().clone(),
    );
    Self {
      role: ctx.signal(profile.map_or("", |p| p.role().as_str()).to_owned()),
      responsibility: ctx.signal(profile.map_or("", |p| p.responsibilities()).to_owned()),
      instructions: ctx.signal(profile.map_or("", |p| p.instructions().as_str()).to_owned()),
      connection: ctx.signal(connection.map_or_else(String::new, |c| c.id().to_string())),
      model: ctx.signal(profile.map_or_else(String::new, |p| p.model().as_str().to_owned())),
      custom: ctx.signal(String::new()),
      changed_connection: ctx.signal(false),
      model_open: ctx.signal(false),
      connection_open: ctx.signal(false),
      effort_open: ctx.signal(false),
      effort: ctx.signal(profile.and_then(stored_effort).unwrap_or_default()),
      permissions: ctx.signal(Permissions(permissions)),
      servers: ctx.signal(Servers(
        profile.map_or_else(McpServerSelection::default, |p| p.mcp_servers().clone()),
      )),
    }
  }

  /// Keeps the fields consistent as the person edits them: a new connection takes its first model and resets what it
  /// cannot enforce, and a new model starts at the default effort.
  pub fn follow(&self, ctx: &mut Ctx, listed: ProjectAgents) {
    let fields = self.clone();
    ctx.watch(&self.connection, move |id| {
      let connection = listed.connections.iter().find(|c| c.id().to_string() == *id);
      fields.model.set(
        connection
          .and_then(first_model)
          .map_or_else(String::new, |model| model.as_str().to_owned()),
      );
      fields.changed_connection.set(true);
      if let Some(connection) = connection {
        let (permissions, servers) = access_for_connection(
          &fields.permissions.get_untracked().0,
          &fields.servers.get_untracked().0,
          connection.provider(),
          Platform::current(),
        );
        fields.permissions.set(Permissions(permissions));
        fields.servers.set(Servers(servers));
      }
    });
    let effort = self.effort.clone();
    ctx.watch(&self.model, move |_| effort.set(String::new()));
  }

  /// Runs `edited` after every edit of what the profile is made of.
  pub fn on_edit(&self, ctx: &mut Ctx, edited: impl Fn() + Send + Sync + 'static) {
    let edited = Arc::new(edited);
    for signal in [
      &self.role,
      &self.responsibility,
      &self.instructions,
      &self.connection,
      &self.model,
      &self.custom,
      &self.effort,
    ] {
      let edited = edited.clone();
      ctx.watch(signal, move |_| edited());
    }
    let changed = edited.clone();
    ctx.watch(&self.permissions, move |_| changed());
    ctx.watch(&self.servers, move |_| edited());
  }

  pub fn connection<'a>(&self, listed: &'a ProjectAgents) -> Option<&'a AgentConnection> {
    let id = self.connection.get();
    listed.connections.iter().find(|c| c.id().to_string() == id)
  }

  pub fn model_value(&self) -> String {
    let selected = self.model.get();
    if selected.is_empty() {
      self.custom.get()
    } else {
      selected
    }
  }

  /// The profile the fields make; reading it in a render re-renders on every edit.
  pub fn profile(&self, listed: &ProjectAgents) -> Result<AgentProfile, Box<ProfileErrors>> {
    let read = Read::Tracked;
    self.form(listed, read)
  }

  /// [`Self::profile`] outside a render.
  pub fn profile_untracked(&self, listed: &ProjectAgents) -> Result<AgentProfile, Box<ProfileErrors>> {
    self.form(listed, Read::Untracked)
  }

  fn form(&self, listed: &ProjectAgents, read: Read) -> Result<AgentProfile, Box<ProfileErrors>> {
    let text = |signal: &Signal<String>| match read {
      Read::Tracked => signal.get(),
      Read::Untracked => signal.get_untracked(),
    };
    let model = match text(&self.model) {
      selected if selected.is_empty() => text(&self.custom),
      selected => selected,
    };
    let effort = text(&self.effort);
    let id = text(&self.connection);
    let (permissions, servers) = match read {
      Read::Tracked => (self.permissions.get().0, self.servers.get().0),
      Read::Untracked => (self.permissions.get_untracked().0, self.servers.get_untracked().0),
    };
    ProfileForm {
      role: &text(&self.role),
      responsibilities: &text(&self.responsibility),
      instructions: &text(&self.instructions),
      connection: listed.connections.iter().find(|c| c.id().to_string() == id),
      model: &ModelSelection::Custom,
      custom_model: &model,
      effort: if effort.is_empty() { None } else { Some(&effort) },
      permissions: &permissions,
      mcp_servers: &servers,
    }
    .profile()
  }
}

#[derive(Clone, Copy)]
enum Read {
  Tracked,
  Untracked,
}
