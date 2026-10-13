//! *Connection & model* (`Section · Connection & model` `KBx06` in `jmyWu`): 760 wide, the project's connection, the
//! model and the reasoning effort as form rows with 320-wide selects, and a note (a 12 `pin` and 12 `text_muted`)
//! under them. A connection the Orchestrator cannot run on is listed, not chosen, with why; without one it can, a
//! warning says so over the section (`No connection can run the Orchestrator` in `qAq0z`) and the model and effort
//! wait for a connection; a model the connection does not list keeps the error stroke and says so.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{connection::ModelSupport, id::ConnectionId, resource::ModelId};
use orchester_workspace::orchestrator::settings::{ConnectionChoice, OrchestratorSetup};

use super::Choices;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsView, ProjectPage, Route},
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{SelectField, field_error},
    form_section::{Label, Place, section, side_row},
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// The form's width (`Form` `Gg1h7`) and a select's (`x2yR47`).
const WIDTH: f32 = 760.0;
const SELECT_WIDTH: f32 = 320.0;

pub(super) fn form(ctx: &mut Ctx, project: &ShownProject, setup: &OrchestratorSetup, choices: &Choices) -> Element {
  let connections = setup.connections();
  let usable = connections.iter().any(|choice| choice.unavailable.is_none());
  let chosen: Option<ConnectionId> = choices.connection.get().parse().ok();
  let connection = connection_row(ctx, &connections, choices, usable);
  let (model, effort) = match chosen {
    Some(chosen) => (
      model_row(ctx, setup, chosen, choices),
      effort_row(ctx, setup, chosen, choices),
    ),
    None => (
      locked_row(
        ctx,
        ("orchestrator-model", "orchestrator.settings.model.label"),
        &choices.model,
      ),
      locked_row(
        ctx,
        ("orchestrator-effort", "orchestrator.settings.effort.label"),
        &choices.effort,
      ),
    ),
  };
  let note = Row::new()
    .id("orchestrator-settings-note")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Pin, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("orchestrator.settings.note"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    );
  let section = section(
    "orchestrator-settings-connection",
    &ctx.t("orchestrator.settings.section"),
    None,
    vec![connection, model, effort],
  );
  Column::new()
    .id("orchestrator-settings-form")
    .width(Dimension::Px(WIDTH))
    .max_width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .with_children((!usable).then(|| no_connection(ctx, project)))
    .child(section)
    .child(note)
    .into()
}

fn connection_row(ctx: &mut Ctx, connections: &[ConnectionChoice], choices: &Choices, usable: bool) -> Element {
  let options = connections
    .iter()
    .map(|choice| {
      let option = SelectOption::new(choice.id.to_string(), choice.name.clone());
      match &choice.unavailable {
        Some(reason) => option.detail(message_text(ctx, reason)).disabled(true),
        None => option,
      }
    })
    .collect();
  let label = ctx.t("orchestrator.settings.connection.label");
  let mut select = SelectField::new(
    "orchestrator-connection",
    label.clone(),
    choices.connection.clone(),
    options,
  )
  .placeholder(ctx.t("orchestrator.settings.connection.none"))
  .width(Dimension::Px(SELECT_WIDTH));
  if !usable {
    select = select.locked();
  }
  side_row(
    Label::new("orchestrator-connection", label).description(ctx.t("orchestrator.settings.connection.description")),
    select.render(ctx),
    Place::Within,
    false,
  )
}

fn model_row(ctx: &mut Ctx, setup: &OrchestratorSetup, connection: ConnectionId, choices: &Choices) -> Element {
  let current = ModelId::new(&choices.model.get()).ok();
  let models = setup.models(connection, current.as_ref());
  let unlisted = models
    .iter()
    .any(|option| Some(&option.id) == current.as_ref() && option.support == ModelSupport::NotVerified);
  let options = models
    .iter()
    .map(|option| SelectOption::new(option.id.as_str().to_owned(), option.id.as_str()))
    .collect();
  let label = ctx.t("orchestrator.settings.model.label");
  let mut select = SelectField::new("orchestrator-model", label.clone(), choices.model.clone(), options)
    .width(Dimension::Px(SELECT_WIDTH));
  if unlisted {
    select = select.invalid();
  }
  let name = setup
    .connections()
    .into_iter()
    .find(|choice| choice.id == connection)
    .map_or_else(|| Arc::from(""), |choice| choice.name);
  let control = Column::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::End)
    .child(select.render(ctx))
    .with_children(unlisted.then(|| {
      field_error(
        "orchestrator-model-error",
        &ctx.t_args("orchestrator.settings.model.notListed", [("connection", &*name)]),
      )
    }));
  side_row(
    Label::new("orchestrator-model", label).description(ctx.t("orchestrator.settings.model.description")),
    control,
    Place::Within,
    false,
  )
}

fn effort_row(ctx: &mut Ctx, setup: &OrchestratorSetup, connection: ConnectionId, choices: &Choices) -> Element {
  let efforts = ModelId::new(&choices.model.get())
    .map(|model| setup.efforts(connection, &model))
    .unwrap_or_default();
  let options = std::iter::once(SelectOption::new(
    String::new(),
    ctx.t("orchestrator.settings.effort.default"),
  ))
  .chain(
    efforts
      .into_iter()
      .map(|effort| SelectOption::new(effort.clone(), effort)),
  )
  .collect();
  let label = ctx.t("orchestrator.settings.effort.label");
  let select = SelectField::new("orchestrator-effort", label.clone(), choices.effort.clone(), options)
    .width(Dimension::Px(SELECT_WIDTH));
  side_row(
    Label::new("orchestrator-effort", label).description(ctx.t("orchestrator.settings.effort.description")),
    select.render(ctx),
    Place::Last,
    false,
  )
}

/// A row whose select waits for a connection: locked, empty, saying so.
fn locked_row(ctx: &mut Ctx, (id, label): (&str, &str), value: &lurq::core::Signal<String>) -> Element {
  let label = ctx.t(label);
  let select = SelectField::new(id.to_owned(), label.clone(), value.clone(), Vec::new())
    .placeholder(ctx.t("orchestrator.dash"))
    .width(Dimension::Px(SELECT_WIDTH))
    .locked();
  side_row(
    Label::new(id, label).description(ctx.t("orchestrator.settings.chooseConnectionFirst")),
    select.render(ctx),
    Place::Within,
    false,
  )
}

/// No connection of the project can run the Orchestrator: why, and Connections.
fn no_connection(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let name = project.open.project.name().as_str();
  let open = ActionButton::new(
    "orchestrator-settings-open-connections",
    ButtonKind::Secondary,
    ctx.t("orchestrator.settings.noConnection.open"),
  )
  .icon(Icon::Plug)
  .on_click(move || navigation.go(Route::Project(id, ProjectPage::Connections(ConnectionsView::default()))));
  Notice::new(
    "orchestrator-settings-no-connection",
    NoticeTone::Warning,
    ctx.t_args("orchestrator.settings.noConnection.title", [("project", name)]),
  )
  .icon(Icon::Unplug)
  .body(ctx.t("orchestrator.settings.noConnection.body"))
  .action(open)
  .into()
}
