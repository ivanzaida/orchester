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
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::ModelSupport,
  id::ConnectionId,
  orchestrator::{MAX_TURNS, TIME_LIMIT},
  resource::ModelId,
};
use orchester_workspace::orchestrator::settings::{ConnectionChoice, OrchestratorSetup};

use super::Choices;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsView, ProjectPage, Route},
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField, field_error},
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
  let connection = connection_row(ctx, &connections, choices);
  let (model, effort) = match chosen {
    Some(chosen) => (
      model_row(ctx, setup, chosen, choices),
      effort_row(ctx, setup, chosen, choices),
    ),
    None => (
      locked_row(ctx, ("orchestrator-model", "orchestrator.settings.model.label")),
      locked_row(ctx, ("orchestrator-effort", "orchestrator.settings.effort.label")),
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
    .child(limits(ctx, project.open.project.name().as_str()))
    .into()
}

/// What the Orchestrator's answers count toward and how long one may run.
fn limits(ctx: &Ctx, project: &str) -> Row {
  let (turns, minutes) = (MAX_TURNS.to_string(), (TIME_LIMIT.as_secs() / 60).to_string());
  Row::new()
    .id("orchestrator-settings-limits")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Gauge, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t_args(
        "orchestrator.settings.limits",
        [("project", project), ("turns", turns.as_str()), ("minutes", minutes.as_str())],
      ))
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted),
    )
}

fn connection_row(ctx: &mut Ctx, connections: &[ConnectionChoice], choices: &Choices) -> Element {
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
  // At full contrast even when none is usable: its menu says why each one is not (`jmyWu`).
  let select = SelectField::new(
    "orchestrator-connection",
    label.clone(),
    choices.connection.clone(),
    options,
  )
  .placeholder(ctx.t("orchestrator.settings.connection.none"))
  .width(Dimension::Px(SELECT_WIDTH));
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
      .map(|effort| {
        let word = effort_word(ctx, &effort);
        SelectOption::new(effort, word)
      }),
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

/// The word for reasoning effort `effort`, as the agent profile editor says it; an effort with no word of its own
/// shows as the CLI names it.
fn effort_word(ctx: &Ctx, effort: &str) -> Arc<str> {
  match effort {
    "minimal" => ctx.t("orchestrator.settings.effort.minimal"),
    "low" => ctx.t("orchestrator.settings.effort.low"),
    "medium" => ctx.t("orchestrator.settings.effort.medium"),
    "high" => ctx.t("orchestrator.settings.effort.high"),
    "xhigh" => ctx.t("orchestrator.settings.effort.xhigh"),
    "max" => ctx.t("orchestrator.settings.effort.max"),
    other => other.into(),
  }
}

/// A row whose choice waits for a connection: a locked box with a dash and a lock, as a locked select shows
/// (`H8GYVZ`), saying so.
fn locked_row(ctx: &mut Ctx, (id, label): (&str, &str)) -> Element {
  let label = ctx.t(label);
  let locked = FieldInput::new(id.to_owned(), label.clone(), Signal::new(ctx.t("orchestrator.dash").to_string()))
    .locked()
    .trailing(Icon::Lock)
    .width(Dimension::Px(SELECT_WIDTH));
  side_row(
    Label::new(id, label).description(ctx.t("orchestrator.settings.chooseConnectionFirst")),
    locked.render(ctx),
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
    ButtonKind::Primary,
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
