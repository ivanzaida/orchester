//! New module and Edit module (no frame yet, ORC-48): a card on the Modules list or a module's page with the name,
//! the description, the start and target dates (`YYYY-MM-DD`) and, for a module that exists, its state; *Create
//! module* or *Save changes* writes it through the project's task writer off the UI thread, *Cancel* closes it.
//! Field errors say why under their field; a refused save says why under the actions.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::module::{Module, ModuleDetails, ModuleState};
use orchester_workspace::{
  background,
  messages::Message,
  modules::form::{ModuleForm, ModuleFormErrors},
  sharing::Servers,
  tasks::writer::TaskWriter,
};

use super::{Editing, words};
use crate::{
  project::ShownProject,
  services::Services,
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card_frame},
    field::{FieldInput, SelectField, field, field_error},
    message::message_text,
  },
};

/// The editor's column, as wide as a form's field group.
const EDITOR_WIDTH: f32 = 560.0;
/// A date field: room for `YYYY-MM-DD`.
const DATE_WIDTH: f32 = 160.0;

#[derive(Clone)]
pub struct EditorProps {
  pub project: ShownProject,
  /// The module edited; `None` for a new one.
  pub module: Option<Arc<Module>>,
  pub editing: Signal<Editing>,
}

impl PartialEq for EditorProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.module == other.module
  }
}

impl DevtoolsInspectable for EditorProps {}

/// A module's state as the select holds it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatePick(pub ModuleState);

impl DevtoolsInspectable for StatePick {}

/// What a save writes, and where: over the module as the editor read it, or as a new one.
#[derive(Clone)]
pub struct SaveModule {
  root: PathBuf,
  module: Option<Arc<Module>>,
  details: ModuleDetails,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved(pub Arc<Module>);

impl DevtoolsInspectable for Saved {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused(pub Message);

impl DevtoolsInspectable for Refused {}

/// The module as the fields were filled from it, which a save changes; `None` for a new module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seeded(pub Option<Arc<Module>>);

impl DevtoolsInspectable for Seeded {}

pub struct ModuleEditor {
  name: Signal<String>,
  description: Signal<String>,
  start: Signal<String>,
  target: Signal<String>,
  state: Signal<StatePick>,
  /// `None` until the fields are filled.
  seeded: Signal<Option<Seeded>>,
  /// Field errors are shown once a save was asked for.
  tried: Signal<bool>,
  save: FutureAction<SaveModule, Saved, Refused>,
}

impl Component for ModuleEditor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let servers = Services::of(ctx).servers;
    let save = ctx.future_action(move |args: SaveModule| {
      let servers = servers.clone();
      async move { background::run(move || write(&args, &servers)).await.map_err(Refused)? }
    });
    let props = ctx.props::<EditorProps>().clone();
    let (feed, editing) = (props.project.feed.clone(), props.editing);
    ctx.watch(&save.state(), move |state| {
      if state.is_fulfilled() && state.data.is_some() {
        feed.saved();
        editing.set(Editing::Closed);
      }
    });
    Self {
      name: ctx.signal(String::new()),
      description: ctx.signal(String::new()),
      start: ctx.signal(String::new()),
      target: ctx.signal(String::new()),
      state: ctx.signal(StatePick::default()),
      seeded: ctx.signal(None),
      tried: ctx.signal(false),
      save,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<EditorProps>().clone();
    self.seed(props.module.as_ref());
    let (name, description, start, target) = (
      self.name.get(),
      self.description.get(),
      self.start.get(),
      self.target.get(),
    );
    let form = ModuleForm {
      name: &name,
      description: &description,
      start: &start,
      target: &target,
      state: self.state.get().0,
    };
    let checked = form.details();
    let errors = match (&checked, self.tried.get()) {
      (Err(errors), true) => errors.clone(),
      _ => ModuleFormErrors::default(),
    };
    let title = ctx.t(if props.module.is_some() {
      "modules.form.editTitle"
    } else {
      "modules.form.newTitle"
    });
    card_frame("module-editor", CardSurface::Panel)
      .width(Dimension::Px(EDITOR_WIDTH))
      .padding(SpacingSize::Lg)
      .spacing(SpacingSize::Md)
      .child(
        Text::new(&title)
          .id("module-editor-title")
          .variant(ExtraTypography::FormSection.role()),
      )
      .child(self.fields(ctx, props.module.is_some(), &errors))
      .child(self.actions(ctx, &props, checked.ok()))
  }
}

impl ModuleEditor {
  /// Fills the fields from the module edited, once, and keeps it as what a save changes.
  fn seed(&self, module: Option<&Arc<Module>>) {
    if self.seeded.get_untracked().is_some() {
      return;
    }
    self.seeded.set(Some(Seeded(module.cloned())));
    let Some(module) = module else {
      return;
    };
    let details = module.details();
    let day = |day: Option<chrono::NaiveDate>| day.map(|day| day.to_string()).unwrap_or_default();
    self.name.set(details.name().as_str().to_owned());
    self.description.set(details.description().to_owned());
    self.start.set(day(details.start()));
    self.target.set(day(details.target()));
    self.state.set(StatePick(details.state()));
  }

  fn fields(&self, ctx: &mut Ctx, existing: bool, errors: &ModuleFormErrors) -> Column {
    let text = |ctx: &mut Ctx, (id, key): (&str, &str), value: &Signal<String>, error: Option<&Message>| {
      let mut input = FieldInput::new(id.to_owned(), ctx.t(key), value.clone());
      if let Some(error) = error {
        input = input.error(message_text(ctx, error));
      }
      input
    };
    let name = text(
      ctx,
      ("module-name", "modules.form.name"),
      &self.name,
      errors.name.as_ref(),
    )
    .render(ctx);
    let description = text(
      ctx,
      ("module-description", "modules.form.description"),
      &self.description,
      None,
    )
    .placeholder(ctx.t("modules.form.descriptionPlaceholder"))
    .compact_text_area()
    .render(ctx);
    let date = |ctx: &mut Ctx, (id, key): (&str, &str), value: &Signal<String>, error: Option<&Message>| {
      let input = text(ctx, (id, key), value, error)
        .placeholder(ctx.t("modules.form.datePlaceholder"))
        .mono()
        .width(Dimension::Px(DATE_WIDTH))
        .render(ctx);
      field(id, Some(&ctx.t(key)), input, None)
    };
    let dates = Row::new()
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Start)
      .child(date(
        ctx,
        ("module-start", "modules.form.start"),
        &self.start,
        errors.start.as_ref(),
      ))
      .child(date(
        ctx,
        ("module-target", "modules.form.target"),
        &self.target,
        errors.target.as_ref(),
      ));
    let state = existing.then(|| {
      let options = [ModuleState::Open, ModuleState::Closed]
        .map(|state| SelectOption::new(StatePick(state), words::state(ctx, state)))
        .to_vec();
      let select = SelectField::new("module-state", ctx.t("modules.form.state"), self.state.clone(), options)
        .width(Dimension::Px(DATE_WIDTH))
        .render(ctx);
      field("module-state", Some(&ctx.t("modules.form.state")), select, None)
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(field("module-name", Some(&ctx.t("modules.form.name")), name, None))
      .child(field(
        "module-description",
        Some(&ctx.t("modules.form.description")),
        description,
        None,
      ))
      .child(dates)
      .with_children(state)
  }

  fn actions(&self, ctx: &mut Ctx, props: &EditorProps, details: Option<ModuleDetails>) -> Column {
    let state = self.save.state().get();
    let (save, tried) = (self.save.clone(), self.tried.clone());
    let base = self.seeded.get_untracked().and_then(|Seeded(module)| module);
    let args = details.map(|details| SaveModule {
      root: props.project.open.repo_root.clone(),
      module: base,
      details,
    });
    let label = ctx.t(if props.module.is_some() {
      "modules.form.save"
    } else {
      "modules.form.create"
    });
    let mut submit = ActionButton::new("module-editor-save", ButtonKind::Primary, label).on_click(move || {
      tried.set(true);
      if let Some(args) = args.clone()
        && !save.state().get_untracked().is_pending()
      {
        save.run(args);
      }
    });
    if state.is_pending() {
      submit = submit.loading();
    }
    let editing = props.editing.clone();
    let cancel = ActionButton::new(
      "module-editor-cancel",
      ButtonKind::Secondary,
      ctx.t("modules.form.cancel"),
    )
    .on_click(move || editing.set(Editing::Closed));
    let refused = state
      .error
      .as_ref()
      .filter(|_| state.is_rejected())
      .map(|Refused(message)| field_error("module-editor-error", &message_text(ctx, message)));
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Xs))
      .spacing(SpacingSize::Sm)
      .with_children(refused)
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(submit)
          .child(cancel),
      )
  }
}

/// Creates or changes the module through the project's task writer; a change is refused when the module changed
/// since the editor read it.
fn write(args: &SaveModule, servers: &Servers) -> Result<Saved, Refused> {
  let refused = |error: &orchester_workspace::tasks::writer::WriteError| {
    tracing::warn!(%error, "the module was not saved");
    Refused(error.message())
  };
  let mut writer = TaskWriter::open(&args.root, servers).map_err(|error| refused(&error))?;
  let module = match &args.module {
    Some(base) => writer.update_module(base.id(), base.details(), args.details.clone()),
    None => writer.create_module(args.details.clone()),
  }
  .map_err(|error| refused(&error))?;
  Ok(Saved(Arc::new(module)))
}
