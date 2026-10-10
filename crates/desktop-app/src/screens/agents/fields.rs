//! Editor field values and Profile tab gTrvs.
use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, theme::SpacingSize},
  components::{Column, SelectOption},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, invocation::Platform, mcp::McpServerSelection},
  permission::AgentPermissions,
  resource::AgentProfile,
};
use orchester_workspace::agents::{
  data::{ProfileEntry, ProjectAgents},
  form::{ModelSelection, ProfileErrors, ProfileForm, effort_options, stored_effort},
};

use super::words;
use crate::ui::{
  field::{FieldInput, SelectField},
  form_section::{self, Label, Place},
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
  pub permissions: Signal<Permissions>,
  pub servers: Signal<Servers>,
  changed_connection: Signal<bool>,
}
/// Selects on Profile are 320 wide (Vavi0); the body leaves 320 for its side panel.
const CONTROL_WIDTH: f32 = 320.0;

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
      effort: ctx.signal(profile.and_then(stored_effort).unwrap_or_default()),
      permissions: ctx.signal(Permissions(permissions)),
      servers: ctx.signal(Servers(
        profile.map_or_else(McpServerSelection::default, |p| p.mcp_servers().clone()),
      )),
    }
  }
  pub fn watch_changes(&self, ctx: &mut Ctx, listed: ProjectAgents, pending: Signal<super::actions::Pending>) {
    for signal in [
      &self.role,
      &self.responsibility,
      &self.instructions,
      &self.connection,
      &self.model,
      &self.custom,
      &self.effort,
    ] {
      let pending = pending.clone();
      ctx.watch(signal, move |_| pending.set(super::actions::Pending(None)));
    }
    let cancel = pending.clone();
    ctx.watch(&self.permissions, move |_| cancel.set(super::actions::Pending(None)));
    ctx.watch(&self.servers, move |_| pending.set(super::actions::Pending(None)));
    let fields = self.clone();
    ctx.watch(&self.connection, move |id| {
      let connection = listed.connections.iter().find(|c| c.id().to_string() == *id);
      fields.model.set(
        connection
          .and_then(|c| c.models().first())
          .map_or_else(String::new, |m| m.id().as_str().to_owned()),
      );
      fields.changed_connection.set(true);
      if let Some(connection) = connection {
        let (fitted, _) = fields
          .permissions
          .get_untracked()
          .0
          .fitted_to(connection.provider(), Platform::current());
        fields.permissions.set(Permissions(fitted));
      }
    });
    let effort = self.effort.clone();
    ctx.watch(&self.model, move |_| effort.set(String::new()));
  }
  pub fn connection<'a>(&self, listed: &'a ProjectAgents) -> Option<&'a AgentConnection> {
    let id = self.connection.get();
    listed.connections.iter().find(|c| c.id().to_string() == id)
  }
  pub fn profile(&self, listed: &ProjectAgents) -> Result<AgentProfile, Box<ProfileErrors>> {
    let model = self.model.get();
    let custom = self.custom.get();
    let model = if model.is_empty() { &custom } else { &model };
    let effort = self.effort.get();
    ProfileForm {
      role: &self.role.get(),
      responsibilities: &self.responsibility.get(),
      instructions: &self.instructions.get(),
      connection: self.connection(listed),
      model: &ModelSelection::Custom,
      custom_model: model,
      effort: if effort.is_empty() { None } else { Some(&effort) },
      permissions: &self.permissions.get().0,
      mcp_servers: &self.servers.get().0,
    }
    .profile()
  }
  pub fn render(&self, ctx: &mut Ctx, listed: &ProjectAgents) -> Element {
    let name = self.input(ctx, "agent-role", "agents.role", self.role.clone());
    let responsibility = self.input(
      ctx,
      "agent-responsibility",
      "agents.responsibility",
      self.responsibility.clone(),
    );
    let connection = self.connection_select(ctx, listed);
    let (model, effort) = self.selects(ctx, listed);
    let instructions = FieldInput::new(
      "agent-instructions",
      ctx.t("agents.instructions"),
      self.instructions.clone(),
    )
    .text_area()
    .render(ctx);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xl)
      .child(
        Column::new()
          .child(form_section::side_row(
            label(ctx, "agent-role", "agents.role", "agents.roleHint"),
            name,
            Place::Within,
            false,
          ))
          .child(form_section::stacked_row(
            label(
              ctx,
              "agent-responsibility",
              "agents.responsibility",
              "agents.responsibilityHint",
            ),
            responsibility,
            Place::Last,
          )),
      )
      .child(form_section::section(
        "agent-model-section",
        &ctx.t("agents.connectionModel"),
        None,
        vec![
          form_section::side_row(
            label(ctx, "agent-connection", "agents.connection", "agents.connectionHint"),
            connection,
            Place::Within,
            false,
          ),
          form_section::side_row(
            label(ctx, "agent-model", "agents.model", "agents.modelHint"),
            model,
            Place::Within,
            false,
          ),
          form_section::side_row(
            label(ctx, "agent-effort", "agents.effort", "agents.effortHint"),
            effort,
            Place::Last,
            false,
          ),
          words::note(&ctx.t("agents.assignmentNote")).into(),
          Column::new()
            .with_children(
              self
                .changed_connection
                .get()
                .then(|| words::note(&ctx.t("agents.connectionReset"))),
            )
            .into(),
        ],
      ))
      .child(form_section::section(
        "agent-instructions-section",
        &ctx.t("agents.instructions"),
        Some(&ctx.t("agents.markdown")),
        vec![words::note(&ctx.t("agents.instructionsHint")).into(), instructions],
      ))
      .into()
  }
  fn connection_select(&self, ctx: &mut Ctx, listed: &ProjectAgents) -> Element {
    SelectField::new(
      "agent-connection",
      ctx.t("agents.connection"),
      self.connection.clone(),
      listed
        .connections
        .iter()
        .map(|c| SelectOption::new(c.id().to_string(), c.name().as_str()))
        .collect(),
    )
    .width(Dimension::Px(CONTROL_WIDTH))
    .render(ctx)
  }
  fn input(&self, ctx: &mut Ctx, id: &str, key: &str, signal: Signal<String>) -> Element {
    FieldInput::new(id, ctx.t(key), signal).render(ctx)
  }
  fn selects(&self, ctx: &mut Ctx, listed: &ProjectAgents) -> (Element, Element) {
    let connection = self.connection(listed);
    let mut models: Vec<_> = connection
      .into_iter()
      .flat_map(|c| c.models())
      .map(|m| SelectOption::new(m.id().as_str().to_owned(), m.id().as_str()))
      .collect();
    let current = self.model.get();
    if !current.is_empty() && !connection.is_some_and(|c| c.models().iter().any(|m| m.id().as_str() == current)) {
      models.push(SelectOption::new(current.clone(), current));
    }
    models.push(SelectOption::new(String::new(), ctx.t("agents.otherModel")));
    let model = SelectField::new("agent-model", ctx.t("agents.model"), self.model.clone(), models).render(ctx);
    let mut model = Column::new()
      .width(Dimension::Px(CONTROL_WIDTH))
      .spacing(SpacingSize::Sm)
      .child(model);
    if self.model.get().is_empty() {
      model = model
        .child(FieldInput::new("agent-custom-model", ctx.t("agents.customModel"), self.custom.clone()).render(ctx));
    }
    let mut efforts = vec![SelectOption::new(String::new(), ctx.t("agents.defaultEffort"))];
    if let Some(c) = connection
      && let Ok(id) = orchester_core::resource::ModelId::new(&self.model.get())
    {
      efforts.extend(
        effort_options(c, &id)
          .iter()
          .map(|effort| SelectOption::new(effort.clone(), effort.clone())),
      );
    }
    let effort = SelectField::new("agent-effort", ctx.t("agents.effort"), self.effort.clone(), efforts)
      .width(Dimension::Px(CONTROL_WIDTH))
      .render(ctx);
    (model.into(), effort)
  }
}
pub fn label(ctx: &Ctx, id: &str, title: &str, hint: &str) -> Label {
  Label::new(id, ctx.t(title)).description(ctx.t(hint))
}
