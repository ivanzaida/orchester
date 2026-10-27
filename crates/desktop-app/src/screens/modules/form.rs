//! New module and Edit module as pages in the task form's style (`08 · New module` `yQqcm`, `09 · Edit module`
//! `HrHGi`): a 720 wide composer centred in the scroll area, its parts 20 apart: `← Modules / New module` (or the
//! module's name, opening its page), the name typed as the title, what the module delivers typed under it, and the
//! property chips of the task form (`task_form::chips`): State for a module that exists, Start date and Target date,
//! each opening its control under it, the dates the date picker (`ui::date_picker`) with the other date ruling its
//! range. Under them a save the module refused says why (another module's name, a change made meanwhile). The action
//! bar at the panel's bottom has Cancel and *Create module* or *Save changes*, which turns its loader while it saves;
//! a saved module's page takes the form's place.

use std::sync::Arc;

use chrono::NaiveDate;
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, TextInput},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::module::{Module, ModuleDetails, ModuleState};
use orchester_workspace::modules::form::{DATE_FORMAT, ModuleForm, ModuleFormErrors};

use super::{
  form_chips as chips,
  save::{self, SaveModule},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ModuleListTab, ModuleTab, ProjectPage, Route},
  screens::task_form::{bar, brief::text_styles},
  theme::{ExtraSpacing, IconSize, TaskFormSpacing, TaskFormText},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The composer's column (`Composer` `A9SKPT`), as the task form's.
const FORM_WIDTH: f32 = 720.0;
/// The most lines the description grows to before its lines scroll.
const DESCRIPTION_MAX_LINES: usize = 20;

#[derive(Clone, PartialEq)]
pub struct FormProps {
  pub project: ShownProject,
  /// The module edited; `None` for a new one.
  pub module: Option<Arc<Module>>,
}

impl DevtoolsInspectable for FormProps {}

/// A date the form holds, or none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Day(pub Option<NaiveDate>);

impl DevtoolsInspectable for Day {}

/// The module's state as the State chip holds it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatePick(pub ModuleState);

impl DevtoolsInspectable for StatePick {}

/// The form's fields.
#[derive(Clone)]
pub struct Fields {
  pub name: Signal<String>,
  pub description: Signal<String>,
  pub start: Signal<Day>,
  pub target: Signal<Day>,
  pub state: Signal<StatePick>,
}

pub struct ModuleFormPage {
  fields: Fields,
  /// The module as the fields were filled from it, which a save changes: a change made elsewhere since is refused.
  base: Option<Arc<Module>>,
  chips: chips::Chips,
  /// Field errors are shown once a save was asked for.
  tried: Signal<bool>,
  save: save::SaveAction,
  /// The name's input, which has the focus on a new module once.
  name_input: ElementRef,
  focused: Signal<bool>,
}

impl Component for ModuleFormPage {
  type Props = FormProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<FormProps>().clone();
    let save = save::action(ctx);
    follow_save(ctx, &save, &props);
    let details = props.module.as_ref().map(|module| module.details().clone());
    let fields = Fields {
      name: ctx.signal(
        details
          .as_ref()
          .map(|details| details.name().as_str().to_owned())
          .unwrap_or_default(),
      ),
      description: ctx.signal(
        details
          .as_ref()
          .map(|details| details.description().to_owned())
          .unwrap_or_default(),
      ),
      start: ctx.signal(Day(details.as_ref().and_then(ModuleDetails::start))),
      target: ctx.signal(Day(details.as_ref().and_then(ModuleDetails::target))),
      state: ctx.signal(StatePick(
        details.as_ref().map(ModuleDetails::state).unwrap_or_default(),
      )),
    };
    Self {
      fields,
      base: props.module.clone(),
      chips: chips::Chips::new(ctx),
      tried: ctx.signal(false),
      save,
      name_input: ElementRef::default(),
      focused: ctx.signal(props.module.is_some()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FormProps>().clone();
    if !self.focused.get_untracked() {
      self.focused.set(true);
      ctx.focus(&self.name_input);
    }
    let checked = self.checked();
    let errors = match (&checked, self.tried.get()) {
      (Err(errors), true) => errors.clone(),
      _ => ModuleFormErrors::default(),
    };
    let composer = Column::new()
      .id("module-form")
      .width(Dimension::Px(FORM_WIDTH))
      .spacing(TaskFormSpacing::Composer.role())
      .child(crumbs(ctx, &props))
      .child(self.name(ctx, &errors))
      .child(self.description(ctx))
      .child(chips::row(ctx, &props, &self.fields, &self.chips))
      .with_children(self.refused(ctx, &errors));
    let page = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Xl)
          .bottom(SpacingSize::Xl)
          .left(TaskFormSpacing::Side.role())
          .right(TaskFormSpacing::Side.role()),
      )
      .align_items(Alignment::Center)
      .child(composer);
    Column::new()
      .id("screen-modules")
      .describe("form", if props.module.is_some() { "edit" } else { "new" })
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(
        lurq::components::ScrollVertical::new(page)
          .id("module-form-scroll")
          .flex(1.0),
      )
      .child(self.bar(ctx, &props, checked.ok()))
  }
}

impl ModuleFormPage {
  /// The details the fields hold, or why they cannot be saved.
  fn checked(&self) -> Result<ModuleDetails, ModuleFormErrors> {
    let date = |day: Day| day.0.map(|day| day.format(DATE_FORMAT).to_string()).unwrap_or_default();
    let (name, description) = (self.fields.name.get(), self.fields.description.get());
    let (start, target) = (date(self.fields.start.get()), date(self.fields.target.get()));
    ModuleForm {
      name: &name,
      description: &description,
      start: &start,
      target: &target,
      state: self.fields.state.get().0,
    }
    .details()
  }

  /// The name, typed as the title (`Title field` `A1VuCL`), and why it is refused under it.
  fn name(&self, ctx: &mut Ctx, errors: &ModuleFormErrors) -> Element {
    let [text, placeholder] = text_styles(ctx, TaskFormText::Title.role(), PaletteColor::TextPrimary);
    let input = TextInput::styled(self.fields.name.clone(), text)
      .placeholder_style(placeholder)
      .placeholder(&ctx.t("modules.form.namePlaceholder"))
      .id("module-name")
      .describe("a11y_name", ctx.t("modules.form.name"))
      .width(Dimension::Pct(100.0))
      .caret_color(PaletteColor::TextPrimary)
      .ref_element(self.name_input.clone());
    let error = errors
      .name
      .as_ref()
      .map(|error| field_error("module-name-note", &message_text(ctx, error)));
    Column::new()
      .id("module-name-field")
      .width(Dimension::Pct(100.0))
      .padding(Padding::vertical(SpacingSize::Xs))
      .spacing(ExtraSpacing::Snug.role())
      .child(input)
      .with_children(error)
      .into()
  }

  /// What the module delivers (`Description field` `cXTQY`), growing with its lines.
  fn description(&self, ctx: &mut Ctx) -> Element {
    let [text, placeholder] = text_styles(ctx, TaskFormText::Outcome.role(), PaletteColor::TextSecondary);
    let input = TextInput::styled(self.fields.description.clone(), text)
      .placeholder_style(placeholder)
      .placeholder(&ctx.t("modules.form.descriptionPlaceholder"))
      .id("module-description")
      .describe("a11y_name", ctx.t("modules.form.description"))
      .textarea()
      .rows(1, DESCRIPTION_MAX_LINES)
      .width(Dimension::Pct(100.0))
      .caret_color(PaletteColor::TextPrimary);
    Column::new()
      .id("module-description-field")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().bottom(SpacingSize::Sm))
      .child(input)
      .into()
  }

  /// Why a save was refused, or the date check's words, under the chips.
  fn refused(&self, ctx: &Ctx, errors: &ModuleFormErrors) -> Option<Element> {
    let state = self.save.state().get();
    let message = state
      .error
      .as_ref()
      .filter(|_| state.is_rejected())
      .map(|save::Refused(message)| message.clone())
      .or_else(|| errors.start.clone().or_else(|| errors.target.clone()))?;
    Some(field_error("module-editor-error", &message_text(ctx, &message)).into())
  }

  /// Cancel and *Create module* or *Save changes* (`Action bar` `Aiehi`).
  fn bar(&self, ctx: &mut Ctx, props: &FormProps, details: Option<ModuleDetails>) -> Element {
    let navigation = Navigation::of(ctx);
    let id = props.project.open.project.id();
    let back = match &props.module {
      Some(module) => ProjectPage::Module(module.id(), ModuleTab::default()),
      None => ProjectPage::Modules(ModuleListTab::default()),
    };
    let cancel = ActionButton::new(
      "module-editor-cancel",
      ButtonKind::Secondary,
      ctx.t("modules.form.cancel"),
    )
    .in_form()
    .opens(navigation.to(Route::Project(id, back)));
    let (label, glyph) = match props.module {
      Some(_) => ("modules.form.save", Icon::Check),
      None => ("modules.form.create", Icon::Plus),
    };
    let (save, tried) = (self.save.clone(), self.tried.clone());
    let args = details.map(|details| SaveModule {
      root: props.project.open.repo_root.clone(),
      module: self.base.clone(),
      details,
    });
    let mut submit = ActionButton::new("module-editor-save", ButtonKind::Primary, ctx.t(label))
      .icon(glyph)
      .in_form()
      .on_click(move || {
        tried.set(true);
        if let Some(args) = args.clone()
          && !save.state().get_untracked().is_pending()
        {
          save.run(args);
        }
      });
    if self.save.state().get().is_pending() {
      submit = submit.loading();
    }
    bar::frame()
      .id("module-form-bar")
      .child(Spacer::new().flex(1.0))
      .child(
        Row::new()
          .spacing(ExtraSpacing::Loose.role())
          .align_items(Alignment::Center)
          .child(cancel)
          .child(submit),
      )
      .into()
  }
}

/// A saved module's page takes the form's place, and the project's tasks are read again.
fn follow_save(ctx: &mut Ctx, save: &save::SaveAction, props: &FormProps) {
  let navigation = Navigation::of(ctx);
  let (feed, project) = (props.project.feed.clone(), props.project.open.project.id());
  ctx.watch(&save.state(), move |state| {
    let Some(save::Saved(module)) = state.data.as_ref().filter(|_| state.is_fulfilled()) else {
      return;
    };
    feed.saved();
    navigation.replace(Route::Project(
      project,
      ProjectPage::Module(module.id(), ModuleTab::default()),
    ));
  });
}

/// `← Modules / New module`, or the module's name, opening its page (`Crumb row` `wZMTu`).
fn crumbs(ctx: &mut Ctx, props: &FormProps) -> Element {
  let navigation = Navigation::of(ctx);
  let id = props.project.open.project.id();
  let list = navigation.to(Route::Project(id, ProjectPage::Modules(ModuleListTab::default())));
  let here = match &props.module {
    Some(module) => {
      let page = ProjectPage::Module(module.id(), ModuleTab::default());
      Crumb::to(
        module.name().as_str().to_owned(),
        navigation.to(Route::Project(id, page)),
      )
    }
    None => Crumb::here(ctx.t("modules.form.newTitle")),
  };
  let trail = vec![Crumb::to(ctx.t("modules.page.back"), list), here];
  Row::new()
    .id("module-form-head")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(breadcrumb(ctx, "module-form-crumbs", CrumbSeparator::Slash, trail))
    .into()
}
