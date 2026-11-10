//! An admin's panel of a team command tool, edited in place (`A29 · Agents · Tool · admin · editing` `FrraS`): the
//! head, the chips with *Unsaved changes* while it differs from the version it is made on, *From the team* with what it
//! runs as fields (its description, program, arguments one per line and how long it may run), *On this computer* as
//! the plain panel has it, and the fixed Save/Cancel bar ([`super::super::edit`]). Its exit codes, output limit and
//! environment stay as the version has them. The fields are the command tool editor's ([`CommandText`]), checked by
//! the types they become. An HTTP request or a database query names its secret by id, which has no team form yet, so
//! those tools are not edited here: their panel stays the plain one.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::PaletteColor,
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::Tool;
use orchester_workspace::{
  agents::team_tool::TeamToolSheet,
  messages::Message,
  tools::form::{CommandErrors, CommandText},
};

use super::{Props, chip, computer, from_team};
use crate::{
  screens::{
    hub::area::Area,
    team_hub::{
      archive,
      edit::{Edit, View},
      panel,
      rows::{self, Sub},
      words,
    },
  },
  services::Services,
  ui::{field::FieldInput, message::message_text},
};

/// The arguments' area is at least 64 high, a few lines.
const ARGUMENTS_AREA: f32 = 64.0;

#[derive(Clone, PartialEq)]
pub(super) struct EditorProps {
  pub props: Props,
}
impl DevtoolsInspectable for EditorProps {}

/// The fields of what the tool runs.
#[derive(Clone)]
struct Fields {
  description: Signal<String>,
  program: Signal<String>,
  arguments: Signal<String>,
  timeout: Signal<String>,
}

impl Fields {
  /// Every field takes `tool`'s value; a tool that is not a command leaves them as they are.
  fn load(&self, tool: &Tool) {
    let Some(text) = CommandText::of(tool) else {
      return;
    };
    self.description.set(text.description);
    self.program.set(text.program);
    self.arguments.set(text.arguments);
    self.timeout.set(text.timeout);
  }

  /// The tool the fields make over `base`, which keeps its exit codes, output limit, environment and access; why it
  /// cannot be made, per field.
  fn made(&self, base: &Tool) -> Result<Tool, Box<CommandErrors>> {
    let kept = CommandText::of(base).unwrap_or_else(CommandText::new_tool);
    CommandText {
      description: self.description.get().trim().to_owned(),
      program: self.program.get(),
      arguments: self.arguments.get(),
      timeout: self.timeout.get(),
      ..kept
    }
    .tool(base.access().cloned())
  }
}

pub(super) struct ToolEditor {
  fields: Fields,
  edit: Edit<Tool>,
}

impl Component for ToolEditor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let EditorProps { props } = ctx.props::<EditorProps>().clone();
    let fields = Fields {
      description: ctx.signal(String::new()),
      program: ctx.signal(String::new()),
      arguments: ctx.signal(String::new()),
      timeout: ctx.signal(String::new()),
    };
    let load = {
      let fields = fields.clone();
      Arc::new(move |tool: &Tool| fields.load(tool))
    };
    let edit = Edit::new(
      ctx,
      (props.repo_root.clone(), props.project, props.entry.id),
      props.sheet.edit.as_ref(),
      load,
    );
    Self { fields, edit }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps { props } = ctx.props::<EditorProps>().clone();
    let Some(base) = self.edit.base() else {
      return Element::from(Row::new());
    };
    let checked = self.fields.made(&base.body);
    let errors = checked
      .as_ref()
      .err()
      .map(|errors| (**errors).clone())
      .unwrap_or_default();
    let made = checked.map_err(|errors| waits(ctx, &errors));
    self.edit.keep_up(
      props.sheet.edit.as_ref(),
      made.as_ref().map_or(true, |tool| *tool != base.body),
    );
    let sheet = &props.sheet;
    let view = View {
      made,
      unreached: props.unreached.as_ref(),
      published_by: sheet.published.as_ref().map(|published| published.by.as_str()),
    };
    let Some(parts) = self.edit.parts(ctx, view) else {
      return Row::new().into();
    };
    let now = Services::of(ctx).clock.now();
    let entry = &props.entry;
    let kind = ctx.t_args(
      "team_hub.tool.where",
      [("kind", from_team::kind(ctx, sheet).to_string())],
    );
    let head = panel::head_of(
      entry,
      (
        kind.to_string(),
        words::published_line(ctx, now, entry, sheet.published.as_ref()),
      ),
      archive::menu(ctx, &props.repo_root, entry, props.team.may_change),
    );
    let badges = parts
      .chip
      .into_iter()
      .chain([chip(ctx, &props.listing, entry)])
      .collect();
    let body = panel::body()
      .child(head)
      .with_children(parts.notice)
      .child(panel::chips_of(ctx, entry, badges, panel::used_by(ctx, &sheet.used_by)))
      .child(self.from_team(ctx, sheet, &errors))
      .child(computer::section(ctx, entry, sheet));
    panel::frame(entry, body, Some(parts.bar)).into()
  }
}

impl ToolEditor {
  /// *From the team*: its kind, then its fields; what cannot be saved says why under the field's label.
  fn from_team(&self, ctx: &mut Ctx, sheet: &TeamToolSheet, errors: &CommandErrors) -> Column {
    let error = |message: &Option<Message>| message.as_ref().map(|message| message_text(ctx, message));
    let program_error = error(&errors.program);
    let timeout_error = error(&errors.timeout);
    let arguments_error = error(&errors.arguments);
    let rows = vec![
      rows::row(
        "team-hub-from-team-kind",
        &ctx.t("team_hub.tool.kindLabel"),
        Some(Sub::quiet(ctx.t("team_hub.tool.kindSub"))),
        rows::words("team-hub-from-team-kind-value", &from_team::kind(ctx, sheet)),
      ),
      field_row(
        ctx,
        ("team-hub-from-team-description", "team_hub.plain.description"),
        (&self.fields.description, false),
        None,
      ),
      field_row(
        ctx,
        ("team-hub-from-team-program", "team_hub.tool.program"),
        (&self.fields.program, true),
        program_error,
      ),
      Area {
        id: "team-hub-from-team-arguments".to_owned(),
        label: ctx.t("team_hub.tool.arguments"),
        sub: ctx.t("team_hub.tool.argumentsSub"),
        placeholder: ctx.t("team_hub.tool.argumentsPlaceholder"),
        height: ARGUMENTS_AREA,
      }
      .row(ctx, &self.fields.arguments, arguments_error),
      field_row(
        ctx,
        ("team-hub-from-team-timeout", "team_hub.tool.timeoutField"),
        (&self.fields.timeout, false),
        timeout_error,
      ),
    ];
    from_team::section_of(ctx, rows)
  }
}

/// A row of one field: `(id, label key)`, typing into `value` (`mono` for a program), with `error` under its label.
fn field_row(
  ctx: &mut Ctx,
  (id, label): (&str, &str),
  (value, mono): (&Signal<String>, bool),
  error: Option<Arc<str>>,
) -> Row {
  let label = ctx.t(label);
  let mut input = FieldInput::new(format!("{id}-input"), label.clone(), value.clone()).width(Dimension::Pct(100.0));
  if mono {
    input = input.mono();
  }
  let sub = match error {
    Some(words) => {
      input = input.invalid();
      Some(Sub {
        words,
        color: PaletteColor::Danger,
      })
    }
    None => None,
  };
  rows::row(id, &label, sub, input.render(ctx)).align_items(Alignment::Center)
}

/// Why the fields make no tool, in the words of the first thing wrong.
fn waits(ctx: &Ctx, errors: &CommandErrors) -> Arc<str> {
  [
    &errors.program,
    &errors.arguments,
    &errors.timeout,
    &errors.exit_codes,
    &errors.output_limit,
    &errors.environment,
  ]
  .into_iter()
  .find_map(|error| error.as_ref())
  .map_or_else(|| ctx.t("team_hub.edit.fixFields"), |error| message_text(ctx, error))
}
