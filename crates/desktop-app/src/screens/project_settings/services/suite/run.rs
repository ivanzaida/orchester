//! The Run group (`Run` `DP8nu`): a new suite's name first; the program, with whether a run finds it under the field
//! (`Note` `YcUkn`); the arguments one per line in a mono text area; the working folder after a folder glyph; and the
//! variables, each a name, `=` and a plain value or a project secret's name. Each row's label column is 220 (`cbpC3`).

use std::path::Path;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};

use super::{
  super::{
    super::kit::Group,
    editor::{
      fields::{field, input, program_found, with_error},
      variables::variables,
    },
  },
  Commit, View,
  fields::SuiteFields,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::icon::Icon,
};

/// The Run group.
pub(super) fn run_group(ctx: &mut Ctx, fields: &SuiteFields, (view, root): (&View, &Path), commit: &Commit) -> Element {
  let problems = view.problems();
  let mut rows = Vec::new();
  if view.base().is_none() {
    let label = ctx.t("project_settings.services.suite.name");
    let control = input(("settings-suite-name", label.clone()), &fields.name, commit);
    let control = with_error(ctx, control, problems.and_then(|p| p.name.as_ref())).render(ctx);
    rows.push(
      field(
        "settings-suite-name",
        label,
        ctx.t("project_settings.services.suite.nameHint"),
      )
      .render(ctx, control),
    );
  }
  rows.push(program_row(ctx, fields, (view, root), commit));
  let label = ctx.t("project_settings.services.editor.arguments");
  let control = input(("settings-suite-arguments", label.clone()), &fields.arguments, commit)
    .text_area()
    .text_style(ExtraTypography::MonoArea.role());
  let control = with_error(ctx, control, problems.and_then(|p| p.arguments.as_ref())).render(ctx);
  rows.push(
    field(
      "settings-suite-arguments",
      label,
      ctx.t("project_settings.services.editor.argumentsHint"),
    )
    .render(ctx, control),
  );
  let label = ctx.t("project_settings.services.editor.folder");
  let control = input(("settings-suite-folder", label.clone()), &fields.folder, commit)
    .leading(Icon::Folder)
    .placeholder(ctx.t("project_settings.services.editor.folderRoot"));
  let control = with_error(ctx, control, problems.and_then(|p| p.working_folder.as_ref())).render(ctx);
  rows.push(
    field(
      "settings-suite-folder",
      label,
      ctx.t("project_settings.services.editor.folderHint"),
    )
    .render(ctx, control),
  );
  let control = variables(
    ctx,
    ("settings-suite", &fields.variables),
    problems.and_then(|p| p.variables.as_ref()),
    commit,
  );
  rows.push(
    field(
      "settings-suite-variables",
      ctx.t("project_settings.services.editor.variables"),
      ctx.t("project_settings.services.editor.variablesHint"),
    )
    .render(ctx, control),
  );
  Group::new("settings-suite-run-group", ctx.t("project_settings.services.suite.run"))
    .rows(rows)
    .into()
}

/// The program, with whether a run finds it while it holds no mistake.
fn program_row(ctx: &mut Ctx, fields: &SuiteFields, (view, root): (&View, &Path), commit: &Commit) -> Element {
  let label = ctx.t("project_settings.services.editor.program");
  let error = view.problems().and_then(|problems| problems.program.as_ref());
  let note = program_found(ctx, (root, &fields.program.get()), "settings-suite-program-found");
  let control = input(("settings-suite-program", label.clone()), &fields.program, commit);
  let control = with_error(ctx, control, error).render(ctx);
  let note = note.filter(|_| error.is_none());
  field(
    "settings-suite-program",
    label,
    ctx.t("project_settings.services.suite.programHint"),
  )
  .render(
    ctx,
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(control)
      .with_children(note),
  )
}
