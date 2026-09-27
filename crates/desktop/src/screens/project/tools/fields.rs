//! The command tool's fields on the Configuration tab (`WneEO`, drawn for a command): name and
//! description, the program and its arguments with where it runs, when it passes and how much
//! output is kept, and the variables it adds to its environment. A value that cannot be used stays
//! in its field with the reason under it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension},
};

use super::{
  Editing,
  editor::{EditorState, ToolEditor, version_text},
};
use crate::{
  activity::operations,
  agents::data::VersionState,
  messages::Message,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  tools::data::ToolChange,
  ui::{
    button::ActionButton,
    field::{FieldNote, field, styled_input, text_input},
    icon::Icon,
    message_text,
    notice::Notice,
    property::property,
  },
};

/// Field height (`i4PL77`) and the rows the text areas grow between.
const FIELD_HEIGHT: f32 = 42.0;
const AREA_ROWS: (usize, usize) = (2, 8);

impl ToolEditor {
  pub(super) fn fields(&self, ctx: &Ctx, state: &EditorState) -> Vec<Element> {
    let errors = state.errors();
    let description = self.input(
      ctx,
      "tool-description-field",
      "tools.editor.description",
      &self.description,
      None,
    );
    let identity = match &self.context.editing {
      Editing::New => pair(
        self.input(ctx, "tool-name", "tools.editor.name", &self.name, errors.name.as_ref()),
        description,
      ),
      Editing::Tool(_) => description,
    };
    vec![
      identity,
      self.command_section(ctx, (errors.program.as_ref(), errors.arguments.as_ref())),
      self.run_section(ctx, &errors),
      self.environment_section(ctx, errors.environment.as_ref()),
    ]
  }

  /// The program, its arguments and where it runs.
  fn command_section(&self, ctx: &Ctx, (program, arguments): (Option<&Message>, Option<&Message>)) -> Element {
    section(ctx, "tools.editor.command")
      .child(self.mono_input(ctx, "tool-program", "tools.editor.program", &self.program, program))
      .child(hint(ctx, "tools.editor.programHint"))
      .child(self.area(
        ctx,
        ("tool-arguments", "tools.editor.arguments"),
        &self.arguments,
        arguments,
      ))
      .child(hint(ctx, "tools.editor.argumentsHint"))
      .child(property(
        &ctx.t("tools.editor.workingDirectory"),
        &ctx.t("tools.editor.worktree"),
      ))
      .into()
  }

  /// When the step passes and how long and how loud the program may be.
  fn run_section(&self, ctx: &Ctx, errors: &crate::tools::form::CommandErrors) -> Element {
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Step.role())
      .child(Column::new().flex(1.0).child(self.input(
        ctx,
        "tool-timeout",
        "tools.editor.timeout",
        &self.timeout,
        errors.timeout.as_ref(),
      )))
      .child(Column::new().flex(1.0).child(self.input(
        ctx,
        "tool-exit-codes",
        "tools.editor.exitCodes",
        &self.exit_codes,
        errors.exit_codes.as_ref(),
      )))
      .child(Column::new().flex(1.0).child(self.input(
        ctx,
        "tool-output-limit",
        "tools.editor.outputLimit",
        &self.output_limit,
        errors.output_limit.as_ref(),
      )));
    section(ctx, "tools.editor.run")
      .child(row)
      .child(hint(ctx, "tools.editor.runHint"))
      .into()
  }

  fn environment_section(&self, ctx: &Ctx, error: Option<&Message>) -> Element {
    section(ctx, "tools.editor.environment")
      .child(self.area(
        ctx,
        ("tool-environment", "tools.editor.variables"),
        &self.environment,
        error,
      ))
      .child(hint(ctx, "tools.editor.environmentHint"))
      .into()
  }

  fn input(
    &self,
    ctx: &Ctx,
    id: &'static str,
    label: &str,
    value: &Signal<String>,
    error: Option<&Message>,
  ) -> Element {
    self.typed_input(ctx, (id, label), value, error, TypographyStyle::FieldLabel)
  }

  fn mono_input(
    &self,
    ctx: &Ctx,
    id: &'static str,
    label: &str,
    value: &Signal<String>,
    error: Option<&Message>,
  ) -> Element {
    self.typed_input(ctx, (id, label), value, error, ExtraTypography::MonoField.role())
  }

  fn typed_input(
    &self,
    ctx: &Ctx,
    (id, label): (&'static str, &str),
    value: &Signal<String>,
    error: Option<&Message>,
    typography: TypographyStyle,
  ) -> Element {
    // A blank field only keeps Save disabled; a value the rules reject says why.
    let error = error
      .filter(|_| !value.get_untracked().trim().is_empty())
      .map(|error| message_text(ctx, error));
    let input = text_input(ctx, id, value.clone(), error.is_some(), typography).height(Dimension::Px(FIELD_HEIGHT));
    field(
      &ctx.t(label),
      input.into(),
      error.map_or(FieldNote::None, FieldNote::Error),
    )
  }

  /// A mono text area drawn like a field, for one value per line.
  fn area(
    &self,
    ctx: &Ctx,
    (id, label): (&'static str, &str),
    value: &Signal<String>,
    error: Option<&Message>,
  ) -> Element {
    let input = styled_input(ctx, value.clone(), ExtraTypography::MonoField.role())
      .id(id)
      .textarea()
      .rows(AREA_ROWS.0, AREA_ROWS.1)
      .width(Dimension::Pct(100.0));
    // Inset like the text of a single-line field.
    let area = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(
        BorderSize::Sm,
        if error.is_some() {
          PaletteColor::BorderStrong
        } else {
          PaletteColor::Border
        },
      ))
      .rounded(ExtraRadius::Notice.role())
      .child(input);
    let note = error.map_or(FieldNote::None, |error| FieldNote::Error(message_text(ctx, error)));
    field(&ctx.t(label), area.into(), note)
  }

  /// A draft started from an older version cannot be published as is (`PublishError::StaleDraft`);
  /// the user decides whether it replaces the newer version.
  pub(super) fn stale_notice(&self, ctx: &Ctx, state: &EditorState) -> Option<Element> {
    let Editing::Tool(entry) = &self.context.editing else {
      return None;
    };
    let VersionState::StaleDraft { based_on, published } = entry.state else {
      return None;
    };
    let change = ToolChange::KeepDraft(entry.id);
    let spec = operations::tool_change(&self.context.project, &entry.name, &change, 0);
    let (action, repo_root) = (self.context.change.clone(), self.context.project.repo_root.clone());
    let keep = ActionButton::new(ctx.t("tools.keepDraft"))
      .icon(Icon::FilePenLine)
      .id("tool-keep-draft")
      .in_form();
    let keep = if state.saving || state.dirty {
      keep.on_click(|| {}).enabled(false)
    } else {
      keep.on_click(move || action.run((repo_root.clone(), change.clone(), spec.clone())))
    };
    let detail = ctx.t_args(
      "tools.staleDetail",
      [
        ("basedOn", &*version_text(ctx, based_on)),
        ("published", &*version_text(ctx, published)),
      ],
    );
    Some(
      Notice::new(Icon::AlertCircle, ctx.t("tools.staleTitle"), detail)
        .emphasized()
        .id("tool-stale-draft")
        .action(keep.render(ctx))
        .into(),
    )
  }
}

/// A group of fields under its heading (`qbbnw`).
fn section(ctx: &Ctx, heading: &str) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Group.role())
    .child(Text::new(&ctx.t(heading)).variant(ExtraTypography::SectionHeading.role()))
}

fn hint(ctx: &Ctx, key: &str) -> Text {
  Text::new(&ctx.t(key)).variant(TypographyStyle::Caption)
}

fn pair(left: Element, right: Element) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Step.role())
    .child(Column::new().flex(1.0).child(left))
    .child(Column::new().flex(1.0).child(right))
    .into()
}
