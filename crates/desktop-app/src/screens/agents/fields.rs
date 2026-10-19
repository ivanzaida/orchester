//! Editor field values and Profile tab gTrvs.
use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, theme::SpacingSize},
  components::Column,
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
  form::{ModelSelection, ProfileErrors, ProfileForm, access_for_connection, effort_options, stored_effort},
};

use super::words;
use crate::ui::{
  field::FieldInput,
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
  model_open: Signal<bool>,
  connection_open: Signal<bool>,
  effort_open: Signal<bool>,
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
  pub fn profile(&self, listed: &ProjectAgents) -> Result<AgentProfile, Box<ProfileErrors>> {
    let model = self.model_value();
    let effort = self.effort.get();
    ProfileForm {
      role: &self.role.get(),
      responsibilities: &self.responsibility.get(),
      instructions: &self.instructions.get(),
      connection: self.connection(listed),
      model: &ModelSelection::Custom,
      custom_model: &model,
      effort: if effort.is_empty() { None } else { Some(&effort) },
      permissions: &self.permissions.get().0,
      mcp_servers: &self.servers.get().0,
    }
    .profile()
  }
  pub fn render(&self, ctx: &mut Ctx, listed: &ProjectAgents) -> Element {
    let name = FieldInput::new("agent-role", ctx.t("agents.role"), self.role.clone())
      .width(Dimension::Px(CONTROL_WIDTH))
      .render(ctx);
    let responsibility = self.input(
      ctx,
      "agent-responsibility",
      "agents.responsibility",
      self.responsibility.clone(),
    );
    let connection = self.connection_select(ctx, listed);
    let (model, effort) = self.selects(ctx, listed);
    // A connection on an API provider offers the models typed for it and no effort (`NNYyh`).
    let on_provider = self.connection(listed).is_some_and(|c| c.api_provider().is_some());
    let (model_hint, effort_hint) = if on_provider {
      ("agents.modelHintApiProvider", "agents.effortHintApiProvider")
    } else {
      ("agents.modelHint", "agents.effortHint")
    };
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
            label(ctx, "agent-model", "agents.model", model_hint),
            model,
            Place::Within,
            false,
          ),
          self.model_notice(ctx, listed),
          form_section::side_row(
            label(ctx, "agent-effort", "agents.effort", effort_hint),
            effort,
            Place::Last,
            false,
          ),
          crate::ui::inspector::note(
            "agent-assignment-note",
            crate::ui::icon::Icon::Pin,
            &ctx.t("agents.assignmentNote"),
          )
          .into(),
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
      .child(self.instructions_section(ctx))
      .into()
  }
  fn instructions_section(&self, ctx: &mut Ctx) -> Element {
    let instructions = FieldInput::new(
      "agent-instructions",
      ctx.t("agents.instructions"),
      self.instructions.clone(),
    )
    .text_area()
    .render(ctx);
    form_section::section(
      "agent-instructions-section",
      &ctx.t("agents.instructions"),
      Some(&ctx.t("agents.markdown")),
      vec![
        Column::new()
          .padding(lurq::node::padding::Padding::new().top(SpacingSize::Md))
          .spacing(crate::theme::ExtraSpacing::Loose.role())
          .child(words::note(&ctx.t("agents.instructionsHint")))
          .child(instructions)
          .into(),
      ],
    )
    .into()
  }
  fn model_notice(&self, ctx: &mut Ctx, listed: &ProjectAgents) -> Element {
    let model = self.model_value();
    let problem = self.connection(listed).and_then(|connection| {
      let model = orchester_core::resource::ModelId::new(&model).ok()?;
      connection
        .cannot_run(&model)
        .map(|problem| (connection.id(), model, problem))
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .with_children(problem.map(|(connection, model, problem)| {
        use crate::{
          routes::{ConnectionsView, ProjectPage, Route},
          ui::{
            button::{ActionButton, ButtonKind},
            icon::Icon,
          },
        };
        let open = self.model_open.clone();
        let nav = crate::navigation::Navigation::of(ctx);
        let project = ctx.props::<super::editor::Props>().project.open.project.id();
        let notice = crate::ui::notice::Notice::new(
          "agent-model-too-old",
          crate::ui::notice::NoticeTone::Warning,
          ctx.t_args(
            "agents.cliTooOld",
            [("installed", problem.installed.as_str()), ("model", model.as_str())],
          ),
        )
        .body(ctx.t_args("agents.cliTooOldHint", [("required", problem.required.as_str())]))
        .action(
          ActionButton::new("agent-choose-model", ButtonKind::Secondary, ctx.t("agents.chooseModel"))
            .on_click(move || open.set(true)),
        )
        .action(
          ActionButton::new(
            "agent-open-connection",
            ButtonKind::Primary,
            ctx.t("agents.openConnection"),
          )
          .icon(Icon::Plug)
          .opens(nav.to(Route::Project(
            project,
            ProjectPage::Connections(ConnectionsView::default().with_selected(connection)),
          ))),
        );
        Column::new()
          .padding(lurq::node::padding::Padding::new().bottom(SpacingSize::Md))
          .child(notice)
      }))
      .into()
  }
  /// The project's connections, each with its CLI and the account it runs on, who is signed in there and the week's
  /// use, or in `warning` what keeps runs from starting (`M42rM`): the profile has no account of its own.
  fn connection_select(&self, ctx: &mut Ctx, listed: &ProjectAgents) -> Element {
    use crate::ui::{
      choice_field::{Choice, ChoiceField},
      icon::Icon,
    };
    let accounts = crate::services::accounts::listed(ctx);
    let readings = crate::services::plan_limits::account_readings(ctx);
    let choices = listed
      .connections
      .iter()
      .map(|c| {
        let (detail, warning) = connection_detail(ctx, c, &accounts, &readings);
        Choice {
          value: c.id().to_string(),
          label: c.name().as_str().into(),
          icon: Icon::Bot,
          detail: (!detail.is_empty()).then(|| detail.into()),
          separator: false,
          warning,
          trigger: None,
          unavailable: None,
        }
      })
      .collect();
    ChoiceField {
      id: "agent-connection".into(),
      name: ctx.t("agents.connection"),
      value: self.connection.clone(),
      open: self.connection_open.clone(),
      choices,
      width: CONTROL_WIDTH,
      icon_in_field: false,
      placeholder: None,
      mono: false,
    }
    .render(ctx)
  }
  fn input(&self, ctx: &mut Ctx, id: &str, key: &str, signal: Signal<String>) -> Element {
    FieldInput::new(id, ctx.t(key), signal).render(ctx)
  }
  fn selects(&self, ctx: &mut Ctx, listed: &ProjectAgents) -> (Element, Element) {
    (self.model_field(ctx, listed), self.effort_field(ctx, listed))
  }
  fn model_field(&self, ctx: &mut Ctx, listed: &ProjectAgents) -> Element {
    use crate::ui::{
      choice_field::{Choice, ChoiceField},
      icon::Icon,
    };
    let connection = self.connection(listed);
    let model_choice = |value: String| Choice {
      label: value.clone().into(),
      value,
      icon: Icon::Sparkles,
      detail: connection.map(|c| c.name().as_str().into()),
      separator: false,
      warning: false,
      trigger: None,
      unavailable: None,
    };
    let mut models: Vec<_> = connection
      .into_iter()
      .flat_map(|c| c.models())
      .map(|m| model_choice(m.id().as_str().to_owned()))
      .collect();
    let current = self.model.get();
    if !current.is_empty() && !models.iter().any(|m| m.value == current) {
      models.push(model_choice(current));
    }
    models.push(Choice {
      value: String::new(),
      label: ctx.t("agents.otherModel"),
      icon: Icon::TextCursorInput,
      detail: Some(ctx.t("agents.otherModelHint")),
      separator: true,
      warning: false,
      trigger: None,
      unavailable: None,
    });
    let model = ChoiceField {
      id: "agent-model".into(),
      name: ctx.t("agents.model"),
      value: self.model.clone(),
      choices: models,
      open: self.model_open.clone(),
      width: CONTROL_WIDTH,
      icon_in_field: false,
      placeholder: None,
      mono: true,
    }
    .render(ctx);
    let mut model = Column::new()
      .width(Dimension::Px(CONTROL_WIDTH))
      .spacing(SpacingSize::Sm)
      .child(model);
    if self.model.get().is_empty() {
      model = model
        .child(FieldInput::new("agent-custom-model", ctx.t("agents.customModel"), self.custom.clone()).render(ctx));
    }
    model.into()
  }
  fn effort_field(&self, ctx: &mut Ctx, listed: &ProjectAgents) -> Element {
    use crate::ui::{
      choice_field::{Choice, ChoiceField},
      icon::Icon,
    };
    let connection = self.connection(listed);
    // The models of an API provider are not known to Orchester: any effort its CLI takes may be typed (`NNYyh`).
    if let Some(cli) = connection.filter(|c| c.api_provider().is_some()).map(|c| c.provider()) {
      let mut field = FieldInput::new("agent-effort", ctx.t("agents.effort"), self.effort.clone())
        .placeholder(ctx.t("agents.providerDefaultEffort"))
        .mono()
        .width(Dimension::Px(CONTROL_WIDTH));
      let typed = self.effort.get();
      if !typed.trim().is_empty()
        && let Err(error) = orchester_core::connection::provider_effort(cli, &typed)
      {
        let key = match error {
          orchester_core::connection::ProviderEffortError::NotOneWord => "agents.effortOneWord",
          orchester_core::connection::ProviderEffortError::NotClaudeLevel => "agents.effortClaudeLevel",
        };
        field = field.error(ctx.t(key));
      }
      return field.render(ctx);
    }
    let mut efforts = vec![Choice {
      value: String::new(),
      label: ctx.t("agents.defaultEffort"),
      icon: Icon::SlidersHorizontal,
      detail: Some(ctx.t("agents.defaultEffortHint")),
      separator: false,
      warning: false,
      trigger: None,
      unavailable: None,
    }];
    if let Some(c) = connection
      && let Ok(id) = orchester_core::resource::ModelId::new(&self.model_value())
    {
      efforts.extend(effort_options(c, &id).iter().map(|effort| {
        let (icon, label) = match effort.as_str() {
          "low" => (Icon::SignalLow, ctx.t("agents.effortLow")),
          "medium" => (Icon::SignalMedium, ctx.t("agents.effortMedium")),
          "high" => (Icon::SignalHigh, ctx.t("agents.effortHigh")),
          _ => (Icon::SignalHigh, effort.as_str().into()),
        };
        Choice {
          value: effort.clone(),
          label,
          icon,
          detail: None,
          separator: false,
          warning: false,
          trigger: None,
          unavailable: None,
        }
      }));
    }
    ChoiceField {
      id: "agent-effort".into(),
      name: ctx.t("agents.effort"),
      value: self.effort.clone(),
      choices: efforts,
      open: self.effort_open.clone(),
      width: CONTROL_WIDTH,
      icon_in_field: false,
      placeholder: None,
      mono: false,
    }
    .render(ctx)
  }
}
pub fn label(ctx: &Ctx, id: &str, title: &str, hint: &str) -> Label {
  Label::new(id, ctx.t(title)).description(ctx.t(hint))
}

/// What a connection's choice says under its name (`M42rM`): its CLI and account with who is signed in there and the
/// week's use, or, in `warning`, what keeps runs from starting on it.
fn connection_detail(
  ctx: &Ctx,
  connection: &AgentConnection,
  accounts: &crate::services::accounts::Accounts,
  readings: &orchester_core::connection::plan_limits::PlanReadings,
) -> (String, bool) {
  use crate::services::accounts::{DetailPlace, account_name, identity_detail, short_state};
  if let Some(provider) = connection.api_provider() {
    let text = ctx.t_args("agents.connectionApiProvider", [("host", provider.base_url().host())]);
    return (text.to_string(), false);
  }
  let (Some(cli), Some(account)) = (
    orchester_core::account::AccountCli::of(connection.provider()),
    accounts.of_connection(connection.account(), connection.provider()),
  ) else {
    // A native connection runs on no account of the person's: its name says it.
    return (String::new(), false);
  };
  let cli = ctx
    .t(match cli {
      orchester_core::account::AccountCli::ClaudeCode => "accounts.cli.claudeCode",
      orchester_core::account::AccountCli::Codex => "accounts.cli.codex",
    })
    .to_string();
  let name = account_name(ctx, account);
  if let Some(problem) = short_state(ctx, account) {
    let text = ctx.t_args(
      "agents.connectionProblem",
      [("cli", cli), ("account", name), ("problem", problem)],
    );
    return (text.to_string(), true);
  }
  let detail = identity_detail(
    ctx,
    (account, accounts),
    readings.of_account(account.id()),
    DetailPlace::AfterComma,
  );
  let text = ctx.t_args(
    "agents.connectionDetail",
    [("cli", cli), ("account", name), ("detail", detail)],
  );
  (text.to_string(), false)
}
