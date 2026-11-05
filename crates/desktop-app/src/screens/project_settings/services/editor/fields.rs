//! The page's fields and the Program group (`Program` `lNoeH`): a new service's name first; the program, with whether
//! a start finds it under the field (`Note` `KCvD9`); the arguments one per line in a mono text area; the working
//! folder after a folder glyph; and the variables ([`super::variables`]). Each row's label column is 220 (`m8rIK4`).

use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, theme::PaletteColor},
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::CommandProgram;
use orchester_workspace::{
  agent_testing::editor::{
    form::{ServiceForm, VariableLine},
    order::Field,
    program::{ProgramFound, find_program_in},
  },
  background,
  messages::Message,
};

use super::{
  super::super::kit::{FormField, Group},
  Commit, LABEL_WIDTH, View,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{field::FieldInput, icon::Icon, message::message_text},
};

/// The variables' rows as typed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Lines(pub Vec<VariableLine>);

impl DevtoolsInspectable for Lines {}

/// The services it needs, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Needs(pub Vec<String>);

impl DevtoolsInspectable for Needs {}

/// Where the program typed is found; `None` while it is no program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Found(Option<ProgramFound>);

impl DevtoolsInspectable for Found {}

/// The fields as typed.
#[derive(Clone)]
pub(super) struct Fields {
  pub name: Signal<String>,
  pub program: Signal<String>,
  pub arguments: Signal<String>,
  pub folder: Signal<String>,
  /// Each row's name and value; a row's own fields type into the signals at its place.
  pub variables: Signal<Lines>,
  pub needs: Signal<Needs>,
  /// The service *Add a service* chose, until it is added.
  pub adding: Signal<String>,
}

impl Fields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      name: ctx.signal(String::new()),
      program: ctx.signal(String::new()),
      arguments: ctx.signal(String::new()),
      folder: ctx.signal(String::new()),
      variables: ctx.signal(Lines::default()),
      needs: ctx.signal(Needs::default()),
      adding: ctx.signal(String::new()),
    }
  }

  pub(super) fn fill(&self, form: &ServiceForm) {
    self.name.set(form.name.clone());
    self.program.set(form.program.clone());
    self.arguments.set(form.arguments.clone());
    self.folder.set(form.working_folder.clone());
    self.variables.set(Lines(form.variables.clone()));
    self.needs.set(Needs(form.needs.clone()));
  }

  /// The fields as they are now, without following them.
  pub(super) fn form(&self) -> ServiceForm {
    ServiceForm {
      name: self.name.get_untracked(),
      program: self.program.get_untracked(),
      arguments: self.arguments.get_untracked(),
      working_folder: self.folder.get_untracked(),
      variables: self.variables.get_untracked().0,
      needs: self.needs.get_untracked().0,
    }
  }

  /// Follows every field, so the page renders again as one is typed in.
  pub(super) fn follow(&self) {
    self.name.get();
    self.program.get();
    self.arguments.get();
    self.folder.get();
    self.variables.get();
    self.needs.get();
  }
}

/// The Program group.
pub(super) fn program_group(ctx: &mut Ctx, fields: &Fields, view: &View, commit: &Commit) -> Element {
  let problems = view.problems();
  let mut rows = Vec::new();
  if view.base().is_none() {
    rows.push(name_row(ctx, fields, problems.and_then(|p| p.name.as_ref()), commit));
  }
  rows.push(program_row(ctx, fields, view, commit));
  rows.push(arguments_row(ctx, fields, view, commit));
  rows.push(folder_row(ctx, fields, view, commit));
  rows.push(variables_row(ctx, fields, view, commit));
  Group::new(
    "settings-service-program-group",
    ctx.t("project_settings.services.editor.program"),
  )
  .rows(rows)
  .into()
}

fn field(id: &str, label: Arc<str>, hint: Arc<str>) -> FormField {
  FormField::new(id, label).hint(hint).label_width(LABEL_WIDTH)
}

/// A field typing into `value` in Geist Mono 12 (`EltV2`), saved as it is left.
fn input((id, name): (&str, Arc<str>), value: &Signal<String>, commit: &Commit) -> FieldInput {
  let commit = commit.clone();
  FieldInput::new(id.to_owned(), name, value.clone())
    .mono()
    .on_leave(move || commit())
}

/// `input` saying `error` under it, its stroke `danger`.
fn with_error(ctx: &Ctx, input: FieldInput, error: Option<&Message>) -> FieldInput {
  match error {
    Some(error) => input.error(message_text(ctx, error)),
    None => input,
  }
}

fn name_row(ctx: &mut Ctx, fields: &Fields, error: Option<&Message>, commit: &Commit) -> Element {
  let label = ctx.t("project_settings.services.editor.name");
  let control = input(("settings-service-name", label.clone()), &fields.name, commit);
  let control = with_error(ctx, control, error).render(ctx);
  field(
    "settings-service-name",
    label,
    ctx.t("project_settings.services.editor.nameHint"),
  )
  .render(ctx, control)
}

fn program_row(ctx: &mut Ctx, fields: &Fields, view: &View, commit: &Commit) -> Element {
  let label = ctx.t("project_settings.services.editor.program");
  let error = view.problems().and_then(|problems| problems.program.as_ref());
  let found = found(ctx, &fields.program.get());
  let control = input(("settings-service-program", label.clone()), &fields.program, commit);
  let control = with_error(ctx, control, error).render(ctx);
  let note = found.filter(|_| error.is_none()).map(|found| found_note(ctx, found));
  field(
    "settings-service-program",
    label,
    ctx.t("project_settings.services.editor.programHint"),
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

/// Where a start finds `program`, looked up off the UI thread as it is typed; `None` while that is not known or it
/// is no program.
fn found(ctx: &mut Ctx, program: &str) -> Option<ProgramFound> {
  let root = ctx.props::<super::EditorProps>().tab.project.open.repo_root.clone();
  let found = ctx
    .future((root, program.to_owned()), |(root, program)| async move {
      background::run(move || {
        Found(
          CommandProgram::new(&program)
            .ok()
            .map(|program| find_program_in(&program, &root)),
        )
      })
      .await
    })
    .state()
    .get();
  found.data?.0
}

/// Whether a start finds the program: in `success` where it does, in `warning` where it does not.
fn found_note(ctx: &Ctx, found: ProgramFound) -> Element {
  let key = match found {
    ProgramFound::OnPath => "project_settings.services.editor.found.onPath",
    ProgramFound::NotOnPath => "project_settings.services.editor.found.notOnPath",
    ProgramFound::InRepository => "project_settings.services.editor.found.inRepository",
    ProgramFound::NotInRepository => "project_settings.services.editor.found.notInRepository",
    ProgramFound::AtPath => "project_settings.services.editor.found.atPath",
    ProgramFound::NotAtPath => "project_settings.services.editor.found.notAtPath",
  };
  let tone = if found.found() {
    PaletteColor::Success
  } else {
    PaletteColor::Warning
  };
  Text::new(&ctx.t(key))
    .id("settings-service-program-found")
    .variant(ExtraTypography::Note.role())
    .color(tone)
    .into()
}

fn arguments_row(ctx: &mut Ctx, fields: &Fields, view: &View, commit: &Commit) -> Element {
  let label = ctx.t("project_settings.services.editor.arguments");
  let error = view
    .problems()
    .and_then(|problems| problems.arguments.clone())
    .or_else(|| placeholder_in(view, Field::Arguments));
  let control = input(("settings-service-arguments", label.clone()), &fields.arguments, commit)
    .text_area()
    .text_style(ExtraTypography::MonoArea.role());
  let control = with_error(ctx, control, error.as_ref()).render(ctx);
  field(
    "settings-service-arguments",
    label,
    ctx.t("project_settings.services.editor.argumentsHint"),
  )
  .render(ctx, control)
}

/// The placeholder problem said under `field`, if it is there.
fn placeholder_in(view: &View, field: Field) -> Option<Message> {
  view
    .placeholder
    .as_ref()
    .filter(|(at, _)| *at == field)
    .map(|(_, message)| message.clone())
}

fn folder_row(ctx: &mut Ctx, fields: &Fields, view: &View, commit: &Commit) -> Element {
  let label = ctx.t("project_settings.services.editor.folder");
  let error = view.problems().and_then(|problems| problems.working_folder.as_ref());
  let control = input(("settings-service-folder", label.clone()), &fields.folder, commit)
    .leading(Icon::Folder)
    .placeholder(ctx.t("project_settings.services.editor.folderRoot"));
  let control = with_error(ctx, control, error).render(ctx);
  field(
    "settings-service-folder",
    label,
    ctx.t("project_settings.services.editor.folderHint"),
  )
  .render(ctx, control)
}

fn variables_row(ctx: &mut Ctx, fields: &Fields, view: &View, commit: &Commit) -> Element {
  let label = ctx.t("project_settings.services.editor.variables");
  let error = view
    .problems()
    .and_then(|problems| problems.variables.clone())
    .or_else(|| placeholder_in(view, Field::Variables));
  let control = super::variables::variables(ctx, &fields.variables, error.as_ref(), commit);
  field(
    "settings-service-variables",
    label,
    ctx.t("project_settings.services.editor.variablesHint"),
  )
  .render(ctx, control)
}
