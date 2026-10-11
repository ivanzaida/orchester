//! The command fields (`zOpcp`), with workspace validation under each value.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{
  messages::Message,
  tools::form::{CommandErrors, CommandText},
};

use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::{
    field::{FieldInput, field},
    form_section::section,
    icon::{Icon, icon},
    message::message_text,
  },
};
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
  pub fn render(&self, ctx: &mut Ctx, errors: &CommandErrors, existing: bool, busy: bool) -> Column {
    let name = self.input(ctx, "name", self.name.clone(), &errors.name, existing || busy);
    // Name `t8jJ9h`: fixed 220, description gets the remaining room.
    const NAME_WIDTH: f32 = 220.0;
    let description = self.input(ctx, "descriptionLabel", self.description.clone(), &None, busy);
    let identity = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(Column::new().width(Dimension::Px(NAME_WIDTH)).child(name))
      .child(Column::new().flex(1.0).child(description));
    let program = self.input(ctx, "program", self.program.clone(), &errors.program, busy);
    let arguments = self.input(ctx, "arguments", self.arguments.clone(), &errors.arguments, busy);
    let runfields = self.run_fields(ctx, errors, busy);
    let variables = self.input(ctx, "variables", self.environment.clone(), &errors.environment, busy);
    Column::new()
      .id("tool-fields")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tabs.role())
      .child(identity)
      .child(
        section(
          "tool-command",
          &ctx.t("tools.commandSection"),
          None,
          vec![
            Column::new()
              .spacing(ExtraSpacing::Loose.role())
              .child(program)
              .child(arguments)
              .child(field(
                "tool-working-directory",
                Some(&ctx.t("tools.workingDirectory")),
                Row::new()
                  .spacing(ExtraSpacing::Pair.role())
                  .align_items(Alignment::Center)
                  .child(icon(Icon::FolderGit2, IconSize::Md, PaletteColor::TextMuted))
                  .child(Text::new(&ctx.t("tools.worktree")).variant(TypographyStyle::Description)),
                None,
              ))
              .into(),
          ],
        )
        .spacing(ExtraSpacing::Loose.role()),
      )
      .child(
        section(
          "tool-run",
          &ctx.t("tools.run"),
          None,
          vec![
            Column::new()
              .spacing(ExtraSpacing::Loose.role())
              .child(runfields)
              .child(Text::new(&ctx.t("tools.runHint")).variant(TypographyStyle::Caption))
              .into(),
          ],
        )
        .spacing(ExtraSpacing::Loose.role()),
      )
      .child(
        section(
          "tool-environment-section",
          &ctx.t("tools.environment"),
          None,
          vec![variables],
        )
        .spacing(ExtraSpacing::Loose.role()),
      )
  }
  fn run_fields(&self, ctx: &mut Ctx, errors: &CommandErrors, busy: bool) -> Row {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(
        Column::new()
          .flex(1.0)
          .child(self.input(ctx, "timeout", self.timeout.clone(), &errors.timeout, busy)),
      )
      .child(Column::new().flex(1.0).child(self.input(
        ctx,
        "exitCodes",
        self.exit_codes.clone(),
        &errors.exit_codes,
        busy,
      )))
      .child(Column::new().flex(1.0).child(self.input(
        ctx,
        "outputLimit",
        self.output_limit.clone(),
        &errors.output_limit,
        busy,
      )))
  }
  fn input(&self, ctx: &mut Ctx, key: &str, value: Signal<String>, error: &Option<Message>, locked: bool) -> Element {
    let id = match key {
      "descriptionLabel" => "description",
      "variables" => "environment",
      other => other,
    };
    let id = format!("tool-{id}");
    let label = ctx.t(&format!("tools.{key}"));
    let mut input = FieldInput::new(id.clone(), label.clone(), value);
    if matches!(key, "arguments" | "variables") {
      input = input.compact_text_area();
    }
    if !matches!(key, "name" | "descriptionLabel") {
      input = input.mono();
    }
    if locked {
      input = input.locked();
      if key == "name" {
        input = input.trailing(Icon::Lock);
      }
    }
    if error.is_some() {
      input = input.invalid();
    }
    let hint = match key {
      "name" if locked => Some("tools.nameHint"),
      "program" => Some("tools.programHint"),
      "arguments" => Some("tools.argumentsHint"),
      "variables" => Some("tools.environmentHint"),
      _ => None,
    };
    let control = input.render(ctx);
    let hint = error.is_none().then(|| hint.map(|key| ctx.t(key))).flatten();
    field(&id, Some(&label), control, hint.as_deref())
      .with_children(error.as_ref().map(|error| {
        Text::new(&message_text(ctx, error))
          .id(format!("{id}-note"))
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::Danger)
      }))
      .into()
  }
}
