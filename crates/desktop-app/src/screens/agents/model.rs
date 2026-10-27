//! A profile page's Role and Connection & model sections (`Role` `ohS4S`, `Connection & model` `e8OCe` in 01b): the
//! name and responsibility fields and the role instructions inline, then the connection, the model and the reasoning
//! effort as dropdowns of one width (01d). *Other model id…* opens the Model id row under Model (01e); a model that
//! lists no efforts has no effort row.
use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, ProviderEffortError, provider_effort},
  resource::{AgentProfile, ModelId},
};
use orchester_workspace::agents::{data::ProjectAgents, form::effort_options};

use super::{fields::Fields, words};
use crate::{
  routes::{ConnectionsView, ProjectPage, Route},
  screens::hub::parts::{boxed, field, group, head, note, stacked_field},
  ui::{
    button::{ActionButton, ButtonKind},
    choice_field::{Choice, ChoiceField},
    field::FieldInput,
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// A field's and a dropdown's width at a row's end (`vxuTD` in `BzwDA`).
const CONTROL_WIDTH: f32 = 260.0;

/// The Role section. With `unsaved`, a save failed: each field whose value differs from the saved `profile` keeps it
/// with the `danger` stroke and the hint under it.
pub(super) fn role(ctx: &mut Ctx, fields: &Fields, unsaved: Option<(&AgentProfile, Arc<str>)>) -> Element {
  let (saved, unsaved_hint) = unsaved.unzip();
  let hint = |differs: bool| unsaved_hint.clone().filter(|_| differs);
  let (role, responsibility, instructions) = (
    fields.role.get(),
    fields.responsibility.get(),
    fields.instructions.get(),
  );
  let name = with_hint(
    FieldInput::new("agent-role", ctx.t("agents.role"), fields.role.clone()).width(Dimension::Px(CONTROL_WIDTH)),
    hint(saved.is_some_and(|p| p.role().as_str() != role)),
  )
  .render(ctx);
  let responsibility = with_hint(
    FieldInput::new(
      "agent-responsibility",
      ctx.t("agents.responsibility"),
      fields.responsibility.clone(),
    )
    .width(Dimension::Px(CONTROL_WIDTH)),
    hint(saved.is_some_and(|p| p.responsibilities() != responsibility)),
  )
  .render(ctx);
  let instructions = with_hint(
    FieldInput::new(
      "agent-instructions",
      ctx.t("agents.instructions"),
      fields.instructions.clone(),
    )
    .text_area()
    .mono(),
    hint(saved.is_some_and(|p| p.instructions().as_str() != instructions)),
  )
  .render(ctx);
  let rows = [
    field(
      "agent-role",
      &ctx.t("agents.role"),
      Some(&ctx.t("agents.roleHint")),
      name,
    )
    .into(),
    field(
      "agent-responsibility",
      &ctx.t("agents.responsibility"),
      Some(&ctx.t("agents.responsibilityHint")),
      responsibility,
    )
    .into(),
    stacked_field(
      "agent-instructions",
      (&ctx.t("agents.instructions"), Some(&ctx.t("agents.instructionsHint"))),
      instructions,
      Some(&ctx.t("agents.markdown")),
    )
    .into(),
  ];
  group(
    "agent-role-section",
    head("agent-role-section", &ctx.t("agents.roleSection"), None, None),
    boxed("agent-role-rows", rows),
  )
  .into()
}

fn with_hint(input: FieldInput, hint: Option<Arc<str>>) -> FieldInput {
  match hint {
    Some(hint) => input.error(hint),
    None => input,
  }
}

pub(super) fn connection_model(
  ctx: &mut Ctx,
  fields: &Fields,
  listed: &ProjectAgents,
  asked: Option<Element>,
) -> Element {
  let connection = fields.connection(listed);
  let on_provider = connection.is_some_and(|c| c.api_provider().is_some());
  let (model_hint, effort_hint) = if on_provider {
    ("agents.modelHintApiProvider", "agents.effortHintApiProvider")
  } else {
    ("agents.modelHint", "agents.effortHint")
  };
  let accounts = crate::services::accounts::listed(ctx);
  let readings = crate::services::plan_limits::account_readings(ctx);
  let detail = connection.map(|c| words::connection_choice(ctx, c, (&accounts, &readings)).0);
  let fallback = ctx.t("agents.connectionHint");
  let hint = detail
    .as_deref()
    .filter(|d| !d.is_empty())
    .unwrap_or(&fallback)
    .to_owned();
  let select = connection_select(ctx, fields, listed, (&accounts, &readings));
  let mut rows = vec![
    field("agent-connection", &ctx.t("agents.connection"), Some(&hint), select).into(),
    field(
      "agent-model",
      &ctx.t("agents.model"),
      Some(&ctx.t(model_hint)),
      model_select(ctx, fields, connection),
    )
    .into(),
  ];
  if fields.model.get().is_empty() {
    let custom = FieldInput::new("agent-custom-model", ctx.t("agents.customModel"), fields.custom.clone())
      .mono()
      .width(Dimension::Px(CONTROL_WIDTH))
      .render(ctx);
    rows.push(
      field(
        "agent-custom-model",
        &ctx.t("agents.customModel"),
        Some(&ctx.t("agents.otherModelHint")),
        custom,
      )
      .into(),
    );
  }
  if let Some(effort) = effort_field(ctx, fields, connection) {
    rows.push(
      field(
        "agent-effort",
        &ctx.t("agents.effort"),
        Some(&ctx.t(effort_hint)),
        effort,
      )
      .into(),
    );
  }
  rows.extend(asked);
  Column::new()
    .id("agent-model-section")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(group(
      "agent-model-group",
      head("agent-model-group", &ctx.t("agents.connectionModel"), None, None),
      boxed("agent-model-rows", rows),
    ))
    .with_children(too_old(ctx, fields, connection))
    .with_children(fields.changed_connection.get().then(|| {
      note(
        "agent-connection-reset",
        Icon::RotateCcw,
        &ctx.t("agents.connectionReset"),
      )
    }))
    .child(note(
      "agent-assignment-note",
      Icon::Info,
      &ctx.t("agents.assignmentNote"),
    ))
    .into()
}

/// The project's connections, each with its CLI and account (`M42rM`).
fn connection_select(
  ctx: &mut Ctx,
  fields: &Fields,
  listed: &ProjectAgents,
  read: (
    &crate::services::accounts::Accounts,
    &orchester_core::connection::plan_limits::PlanReadings,
  ),
) -> Element {
  let choices = listed
    .connections
    .iter()
    .map(|c| {
      let (detail, warning) = words::connection_choice(ctx, c, read);
      Choice {
        value: c.id().to_string(),
        label: c.name().as_str().into(),
        icon: super::entries::provider_glyph(Some(c)),
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
    value: fields.connection.clone(),
    open: fields.connection_open.clone(),
    choices,
    width: CONTROL_WIDTH,
    icon_in_field: false,
    placeholder: None,
    mono: false,
  }
  .render(ctx)
}

/// The connection's model ids in catalog order, each with the connection, then *Other model id…*.
fn model_select(ctx: &mut Ctx, fields: &Fields, connection: Option<&AgentConnection>) -> Element {
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
  let current = fields.model.get();
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
  ChoiceField {
    id: "agent-model".into(),
    name: ctx.t("agents.model"),
    value: fields.model.clone(),
    choices: models,
    open: fields.model_open.clone(),
    width: CONTROL_WIDTH,
    icon_in_field: false,
    placeholder: None,
    mono: true,
  }
  .render_inline(ctx)
}

/// The effort a model lists as a dropdown, or on an API provider any one word its CLI takes; `None` for a model of a
/// CLI connection that lists no efforts.
fn effort_field(ctx: &mut Ctx, fields: &Fields, connection: Option<&AgentConnection>) -> Option<Element> {
  // The models of an API provider are not known to Orchester: any effort its CLI takes may be typed (`NNYyh`).
  if let Some(cli) = connection.filter(|c| c.api_provider().is_some()).map(|c| c.provider()) {
    let mut field = FieldInput::new("agent-effort", ctx.t("agents.effort"), fields.effort.clone())
      .placeholder(ctx.t("agents.providerDefaultEffort"))
      .mono()
      .width(Dimension::Px(CONTROL_WIDTH));
    let typed = fields.effort.get();
    if !typed.trim().is_empty()
      && let Err(error) = provider_effort(cli, &typed)
    {
      field = field.error(ctx.t(match error {
        ProviderEffortError::NotOneWord => "agents.effortOneWord",
        ProviderEffortError::NotClaudeLevel => "agents.effortClaudeLevel",
      }));
    }
    return Some(field.render(ctx));
  }
  let connection = connection?;
  let model = ModelId::new(&fields.model_value()).ok()?;
  let listed = effort_options(connection, &model);
  if listed.is_empty() && fields.effort.get().is_empty() {
    return None;
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
  efforts.extend(listed.iter().map(|effort| Choice {
    value: effort.clone(),
    label: words::effort(ctx, effort),
    icon: Icon::SignalHigh,
    detail: None,
    separator: false,
    warning: false,
    trigger: None,
    unavailable: None,
  }));
  Some(
    ChoiceField {
      id: "agent-effort".into(),
      name: ctx.t("agents.effort"),
      value: fields.effort.clone(),
      choices: efforts,
      open: fields.effort_open.clone(),
      width: CONTROL_WIDTH,
      icon_in_field: false,
      placeholder: None,
      mono: false,
    }
    .render_inline(ctx),
  )
}

/// *CLI 2.1 is too old for the model*, with Choose another model and Open connection.
fn too_old(ctx: &mut Ctx, fields: &Fields, connection: Option<&AgentConnection>) -> Option<Element> {
  let connection = connection?;
  let model = ModelId::new(&fields.model_value()).ok()?;
  let problem = connection.cannot_run(&model)?;
  let open = fields.model_open.clone();
  let nav = crate::navigation::Navigation::of(ctx);
  let project = ctx.props::<super::editor::Props>().project.open.project.id();
  Some(
    Notice::new(
      "agent-model-too-old",
      NoticeTone::Warning,
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
        ProjectPage::Connections(ConnectionsView::default().with_selected(connection.id())),
      ))),
    )
    .into(),
  )
}
