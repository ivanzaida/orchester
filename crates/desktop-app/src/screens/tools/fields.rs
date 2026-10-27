//! A command tool's fields on its Configuration tab (`04b` `nuoSy`), with workspace validation under each value.
use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{
  messages::Message,
  tools::form::{CommandErrors, CommandText},
};

use super::fields_rows::{Control, Setting, read_row, section, setting};
use crate::{screens::hub::parts::note, theme::HubSpacing, ui::icon::Icon};
#[derive(Clone)]
pub(super) struct Fields {
  pub name: Signal<String>,
  description: Signal<String>,
  program: Signal<String>,
  arguments: Signal<String>,
  timeout: Signal<String>,
  exit_codes: Signal<String>,
  output_limit: Signal<String>,
  environment: Signal<String>,
}
impl Fields {
  pub fn new(ctx: &mut Ctx, name: &str, text: &CommandText) -> Self {
    Self {
      name: ctx.signal(name.to_owned()),
      description: ctx.signal(text.description.clone()),
      program: ctx.signal(text.program.clone()),
      arguments: ctx.signal(text.arguments.clone()),
      timeout: ctx.signal(text.timeout.clone()),
      exit_codes: ctx.signal(text.exit_codes.clone()),
      output_limit: ctx.signal(text.output_limit.clone()),
      environment: ctx.signal(text.environment.clone()),
    }
  }
  pub fn text(&self) -> CommandText {
    CommandText {
      description: self.description.get(),
      program: self.program.get(),
      arguments: self.arguments.get(),
      timeout: self.timeout.get(),
      exit_codes: self.exit_codes.get(),
      output_limit: self.output_limit.get(),
      environment: self.environment.get(),
    }
  }
  /// The Command, Run and Environment sections; a saved tool's name is fixed and only read.
  pub fn render(&self, ctx: &mut Ctx, errors: &CommandErrors, existing: bool, busy: bool) -> Column {
    let name = if existing {
      read_row(
        "name",
        &ctx.t("tools.name"),
        Some(&ctx.t("tools.nameHint")),
        &self.name.get(),
      )
    } else {
      self.setting(ctx, ("name", "tools.name", None), Control::Text, (&errors.name, busy))
    };
    let command = vec![
      name,
      self.setting(
        ctx,
        ("description", "tools.descriptionLabel", None),
        Control::Text,
        (&None, busy),
      ),
      self.setting(
        ctx,
        ("program", "tools.program", Some("tools.programHint")),
        Control::Mono,
        (&errors.program, busy),
      ),
      self.setting(
        ctx,
        ("arguments", "tools.arguments", Some("tools.argumentsHint")),
        Control::Lines,
        (&errors.arguments, busy),
      ),
      read_row(
        "working-directory",
        &ctx.t("tools.workingDirectory"),
        None,
        &ctx.t("tools.worktree"),
      ),
    ];
    let run = vec![
      self.setting(
        ctx,
        ("timeout", "tools.timeout", None),
        Control::Number,
        (&errors.timeout, busy),
      ),
      self.setting(
        ctx,
        ("exitCodes", "tools.exitCodes", None),
        Control::Number,
        (&errors.exit_codes, busy),
      ),
      self.setting(
        ctx,
        ("outputLimit", "tools.outputLimit", None),
        Control::Number,
        (&errors.output_limit, busy),
      ),
    ];
    let environment = vec![self.setting(
      ctx,
      ("environment", "tools.variables", Some("tools.environmentHint")),
      Control::Lines,
      (&errors.environment, busy),
    )];
    Column::new()
      .id("tool-fields")
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Sections.role())
      .child(section("tool-command", &ctx.t("tools.commandSection"), command))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(section("tool-run", &ctx.t("tools.run"), run))
          .child(note("tool-run-hint", Icon::Info, &ctx.t("tools.runHint"))),
      )
      .child(section(
        "tool-environment-section",
        &ctx.t("tools.environment"),
        environment,
      ))
  }

  /// One setting row: its input is `tool-<id>`, named and labelled by `label`, with `hint` under the label unless
  /// the field has an error.
  fn setting(
    &self,
    ctx: &mut Ctx,
    (id, label, hint): (&str, &str, Option<&str>),
    control: Control,
    (error, locked): (&Option<Message>, bool),
  ) -> Element {
    let value = match id {
      "name" => self.name.clone(),
      "description" => self.description.clone(),
      "program" => self.program.clone(),
      "arguments" => self.arguments.clone(),
      "timeout" => self.timeout.clone(),
      "exitCodes" => self.exit_codes.clone(),
      "outputLimit" => self.output_limit.clone(),
      _ => self.environment.clone(),
    };
    let label = ctx.t(label);
    let hint = hint.filter(|_| error.is_none()).map(|key| ctx.t(key));
    setting(
      ctx,
      &Setting {
        id,
        label: &label,
        hint: hint.as_deref(),
        control,
        error,
        locked,
      },
      value,
    )
  }
}
