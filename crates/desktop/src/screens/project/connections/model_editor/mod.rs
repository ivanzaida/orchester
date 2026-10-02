//! A native connection's models as a list that edits one model in place (04.H6 `zHowy`, the models of
//! 04.H1B `h2OUr`): each row shows the id, the *Default* label and what is known of the model, with Edit
//! and Remove; the row being edited is in [`form`]. A model something uses cannot be removed: the row
//! says who must switch first (the user's decisions of 2026-10-01 and 2026-10-02).

mod form;

use std::{num::NonZeroU32, sync::Arc};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{Effort, NativeModel},
  resource::ModelId,
};

use super::{dialog_frame::BUTTON_HEIGHT, model_text};
use crate::{
  connections::model_uses::{ModelUse, ModelUser},
  theme::ExtraTypography,
  ui::{button::ActionButton, icon::Icon, option_card::unavailable_reason, status_label::compact_status_label},
};

/// Height of an effort chip and of a row's Edit and Remove (`bpmzN`, `TkTBZ`).
const CHIP_HEIGHT: f32 = 26.0;

/// The models being edited, in their order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelRows(pub Vec<NativeModel>);

impl lurq::app::component::DevtoolsInspectable for ModelRows {}

/// The model whose row is open for editing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum Editing {
  #[default]
  None,
  New,
  Model(ModelId),
}

impl lurq::app::component::DevtoolsInspectable for Editing {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Efforts(Vec<Effort>);

impl lurq::app::component::DevtoolsInspectable for Efforts {}

#[derive(Clone)]
pub struct ModelEditorProps {
  pub models: Signal<ModelRows>,
  pub default_model: Option<ModelId>,
  pub uses: Arc<Vec<ModelUse>>,
  /// The add button's words: *Add a model by hand* in the connection form, *Add a model* in the
  /// models dialog.
  pub add: &'static str,
}

impl PartialEq for ModelEditorProps {
  // The models signal stays the same while the editor is mounted; its changes re-render the editor.
  fn eq(&self, other: &Self) -> bool {
    self.default_model == other.default_model && self.uses == other.uses && self.add == other.add
  }
}

impl lurq::app::component::DevtoolsInspectable for ModelEditorProps {}

pub struct ModelEditor {
  editing: Signal<Editing>,
  id: Signal<String>,
  context: Signal<String>,
  max_output: Signal<String>,
  efforts: Signal<Efforts>,
}

impl Component for ModelEditor {
  type Props = ModelEditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      editing: ctx.signal(Editing::None),
      id: ctx.signal(String::new()),
      context: ctx.signal(String::new()),
      max_output: ctx.signal(String::new()),
      efforts: ctx.signal(Efforts::default()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ModelEditorProps>().clone();
    let models = props.models.get().0;
    let editing = self.editing.get();
    let mut rows: Vec<Element> = models
      .iter()
      .map(|model| match &editing {
        Editing::Model(id) if id == model.id() => self.editor(ctx, &props, &models),
        _ => self.row(ctx, &props, model, editing == Editing::None),
      })
      .collect();
    if editing == Editing::New {
      rows.push(self.editor(ctx, &props, &models));
    }
    let start = self.start(None);
    let add = ActionButton::new(ctx.t(props.add))
      .icon(Icon::Plus)
      .id("model-add")
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT))
      .on_click(start)
      .enabled(editing == Editing::None)
      .render(ctx);
    Column::new()
      .id("model-list")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .overflow_visible()
      .with_children(rows)
      .child(Row::new().overflow_visible().child(add))
  }
}

impl ModelEditor {
  /// Opens `model`'s row for editing, or a new row without one.
  fn start(&self, model: Option<&NativeModel>) -> impl Fn() + Send + Sync + 'static {
    let (editing, id, context, max_output, efforts) = (
      self.editing.clone(),
      self.id.clone(),
      self.context.clone(),
      self.max_output.clone(),
      self.efforts.clone(),
    );
    let model = model.cloned();
    move || {
      let text = |tokens: Option<NonZeroU32>| tokens.map(|tokens| tokens.get().to_string()).unwrap_or_default();
      id.set(
        model
          .as_ref()
          .map(|model| model.id().as_str().to_owned())
          .unwrap_or_default(),
      );
      context.set(text(model.as_ref().and_then(NativeModel::context_window)));
      max_output.set(text(model.as_ref().and_then(NativeModel::max_output)));
      efforts.set(Efforts(
        model.as_ref().map(|model| model.efforts().to_vec()).unwrap_or_default(),
      ));
      editing.set(
        model
          .as_ref()
          .map_or(Editing::New, |model| Editing::Model(model.id().clone())),
      );
    }
  }

  /// A model as it reads: id, default label, source, limits and efforts, Edit and Remove.
  fn row(&self, ctx: &Ctx, props: &ModelEditorProps, model: &NativeModel, actions: bool) -> Element {
    let id = model.id().as_str();
    let users: Vec<Arc<str>> = props
      .uses
      .iter()
      .filter(|used| &used.model == model.id())
      .map(|used| user_name(ctx, &used.user))
      .collect();
    let title = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(Text::new(id).variant(ExtraTypography::MonoValue.role()))
      .with_children(
        (props.default_model.as_ref() == Some(model.id()))
          .then(|| compact_status_label(Icon::Star, &ctx.t("connections.models.default"))),
      );
    let text = Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Xs)
      .child(title)
      .child(Text::new(&model_text::summary(ctx, model)).variant(ExtraTypography::InspectorNote.role()));
    let edit = small_button(
      ctx,
      format!("model-edit-{id}"),
      "connections.models.editModel",
      Icon::Pencil,
    )
    .on_click(self.start(Some(model)))
    .enabled(actions)
    .render(ctx);
    let remove = users.is_empty().then(|| {
      small_button(
        ctx,
        format!("model-remove-{id}"),
        "connections.models.remove",
        Icon::Trash,
      )
      .on_click(remove(&props.models, model.id().clone()))
      .enabled(actions)
      .render(ctx)
    });
    let line = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .overflow_visible()
      .child(text)
      .child(edit)
      .with_children(remove);
    let separator = ctx.t("common.listSeparator");
    let blocked = (!users.is_empty()).then(|| {
      unavailable_reason(&ctx.t_args(
        "connections.models.inUse",
        [("profiles", users.join(&separator).as_str()), ("model", id)],
      ))
    });
    Column::new()
      .id(format!("model-row-{id}"))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .child(line)
      .with_children(blocked)
      .into()
  }
}

fn remove(models: &Signal<ModelRows>, id: ModelId) -> impl Fn() + Send + Sync + 'static {
  let models = models.clone();
  move || models.update(|rows| rows.0.retain(|model| model.id() != &id))
}

/// A profile by its role, a pipeline step as "Feature delivery · Implement".
fn user_name(ctx: &Ctx, user: &ModelUser) -> Arc<str> {
  match user {
    ModelUser::Profile(role) => role.clone(),
    ModelUser::Step { pipeline, step } => ctx.t_args(
      "connections.models.stepUser",
      [("pipeline", &**pipeline), ("step", &**step)],
    ),
  }
}

fn small_button(ctx: &Ctx, id: String, key: &str, glyph: Icon) -> ActionButton {
  ActionButton::new(ctx.t(key))
    .icon(glyph)
    .compact()
    .id(id)
    .in_form()
    .height(Dimension::Px(CHIP_HEIGHT))
}
