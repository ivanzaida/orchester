//! A native connection's models as a list that edits one model in place (04.H6 `zHowy`, the models of
//! 04.H1B `h2OUr`): each row shows the id, the default label and what is known of the model, with Edit
//! and Remove; the row being edited takes the id, the optional context window and max output, and the
//! effort chips. A model a profile uses cannot be removed: the row says which profiles must switch first
//! (decision of 2026-10-01).

use std::{num::NonZeroU32, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{Effort, ModelSource, NativeModel},
  resource::ModelId,
};

use super::{dialog_frame::BUTTON_HEIGHT, model_text};
use crate::{
  connections::native::{ModelUse, keep_used},
  messages::Message,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{FieldNote, field, text_input},
    icon::Icon,
    message_text,
    option_card::unavailable_reason,
    pressable::{Ring, keyboard_stop},
    status_label::compact_status_label,
  },
};

/// Height of an effort chip and of a row's Edit and Remove (`bpmzN`).
const CHIP_HEIGHT: f32 = 26.0;

/// The models being edited, in their order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelRows(pub Vec<NativeModel>);

impl lurq::app::component::DevtoolsInspectable for ModelRows {}

/// The model whose row is open for editing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Editing {
  #[default]
  None,
  New,
  Model(ModelId),
}

impl lurq::app::component::DevtoolsInspectable for Editing {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Efforts(Vec<Effort>);

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
    let users: Vec<&str> = props
      .uses
      .iter()
      .filter(|used| &used.model == model.id())
      .map(|used| &*used.role)
      .collect();
    let title = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(Text::new(id).variant(ExtraTypography::MonoValue.role()))
      .with_children(
        (props.default_model.as_ref() == Some(model.id()))
          .then(|| compact_status_label(Icon::Check, &ctx.t("connections.models.default"))),
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

  /// The row being edited (`zHowy`): id, limits, efforts, and Remove model, Cancel and Save model.
  fn editor(&self, ctx: &Ctx, props: &ModelEditorProps, models: &[NativeModel]) -> Element {
    let editing = self.editing.get();
    let model = self.model(models, &editing, &props.uses);
    let error = model.as_ref().err().and_then(Clone::clone);
    let limits = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Step.role())
      .child(
        Column::new()
          .flex(2.0)
          .child(self.input(ctx, "model-id", "connections.models.id", &self.id)),
      )
      .child(Column::new().flex(1.0).child(self.input(
        ctx,
        "model-context",
        "connections.models.contextWindow",
        &self.context,
      )))
      .child(Column::new().flex(1.0).child(self.input(
        ctx,
        "model-max-output",
        "connections.models.maxOutputLabel",
        &self.max_output,
      )));
    let chosen = self.efforts.get().0;
    let chips = Row::new()
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .with_children(Effort::ALL.map(|effort| effort_chip(&self.efforts, effort, chosen.contains(&effort))));
    let note = error.map_or(FieldNote::None, |error| FieldNote::Error(message_text(ctx, &error)));
    Column::new()
      .id("model-editor")
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Md)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .overflow_visible()
      .child(limits)
      .child(field(&ctx.t("connections.models.efforts"), chips.into(), note))
      .child(self.editor_actions(ctx, props, &editing, model.ok()))
      .into()
  }

  fn input(&self, ctx: &Ctx, id: &'static str, label: &str, value: &Signal<String>) -> Element {
    let input = text_input(ctx, id, value.clone(), false, ExtraTypography::MonoField.role());
    field(&ctx.t(label), input.into(), FieldNote::None)
  }

  fn editor_actions(
    &self,
    ctx: &Ctx,
    props: &ModelEditorProps,
    editing: &Editing,
    model: Option<NativeModel>,
  ) -> Element {
    let used = |id: &ModelId| props.uses.iter().any(|used| &used.model == id);
    let remove_model = match editing {
      Editing::Model(id) if !used(id) => Some(
        ActionButton::new(ctx.t("connections.models.removeModel"))
          .icon(Icon::Trash)
          .id("model-remove")
          .in_form()
          .height(Dimension::Px(BUTTON_HEIGHT))
          .on_click(remove(&props.models, id.clone()))
          .render(ctx),
      ),
      _ => None,
    };
    let cancel = self.editing.clone();
    let save = ActionButton::new(ctx.t("connections.models.save"))
      .primary()
      .icon(Icon::Check)
      .id("model-save")
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT));
    let save = match model {
      Some(model) => {
        let (models, editing, target) = (props.models.clone(), self.editing.clone(), editing.clone());
        save.on_click(move || {
          models.update(|rows| match &target {
            Editing::Model(id) => {
              if let Some(row) = rows.0.iter_mut().find(|row| row.id() == id) {
                *row = model.clone();
              }
            }
            Editing::New | Editing::None => rows.0.push(model.clone()),
          });
          editing.set(Editing::None);
        })
      }
      None => save.on_click(|| {}).enabled(false),
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .overflow_visible()
      .with_children(remove_model)
      .child(Spacer::new().flex(1.0))
      .child(
        ActionButton::new(ctx.t("common.cancel"))
          .icon(Icon::X)
          .id("model-cancel")
          .in_form()
          .height(Dimension::Px(BUTTON_HEIGHT))
          .on_click(move || cancel.set(Editing::None))
          .render(ctx),
      )
      .child(save.render(ctx))
      .into()
  }

  /// The model the open row would save, or why it cannot be saved: `None` while the id is still
  /// empty, which needs no explanation. An edit that leaves the model as it was keeps its source;
  /// any change makes it a model added by hand. The id of a model a profile uses stays.
  fn model(
    &self,
    models: &[NativeModel],
    editing: &Editing,
    uses: &[ModelUse],
  ) -> Result<NativeModel, Option<Message>> {
    let entered = self.id.get();
    let entered = entered.trim();
    if entered.is_empty() {
      return Err(None);
    }
    let id = ModelId::new(entered).map_err(|_| Some(Message::new("connections.models.invalidId")))?;
    let listed = models.iter().any(|model| model.id() == &id);
    if listed && *editing != Editing::Model(id.clone()) {
      return Err(Some(
        Message::new("connections.models.listedAlready").with("model", id.as_str().to_owned()),
      ));
    }
    let context = tokens(&self.context.get())?;
    let max_output = tokens(&self.max_output.get())?;
    let efforts = self.efforts.get().0;
    let Editing::Model(edited) = editing else {
      return Ok(NativeModel::new(id, context, max_output, efforts, ModelSource::Manual));
    };
    let before = models.iter().find(|model| model.id() == edited);
    let unchanged = before
      .map(|before| NativeModel::new(id.clone(), context, max_output, efforts.clone(), before.source()))
      .filter(|model| Some(model) == before);
    let model = unchanged.unwrap_or_else(|| NativeModel::new(id, context, max_output, efforts, ModelSource::Manual));
    let after: Vec<NativeModel> = models
      .iter()
      .map(|row| if row.id() == edited { model.clone() } else { row.clone() })
      .collect();
    keep_used(&after, uses, "connections.models.idInUse").map_err(Some)?;
    Ok(model)
  }
}

/// An optional token count as typed: empty is unknown, else a whole number above zero.
fn tokens(text: &str) -> Result<Option<NonZeroU32>, Option<Message>> {
  let text = text.trim();
  if text.is_empty() {
    return Ok(None);
  }
  text
    .parse::<NonZeroU32>()
    .map(Some)
    .map_err(|_| Some(Message::new("connections.models.invalidTokens")))
}

fn remove(models: &Signal<ModelRows>, id: ModelId) -> impl Fn() + Send + Sync + 'static {
  let models = models.clone();
  move || models.update(|rows| rows.0.retain(|model| model.id() != &id))
}

fn small_button(ctx: &Ctx, id: String, key: &str, glyph: Icon) -> ActionButton {
  ActionButton::new(ctx.t(key))
    .icon(glyph)
    .compact()
    .id(id)
    .in_form()
    .height(Dimension::Px(CHIP_HEIGHT))
}

/// An effort the model takes or not (`bpmzN`): the chosen ones outlined in the text color.
fn effort_chip(efforts: &Signal<Efforts>, effort: Effort, chosen: bool) -> Element {
  let (stroke, color) = if chosen {
    (BorderSize::Md, PaletteColor::TextPrimary)
  } else {
    (BorderSize::Sm, PaletteColor::Border)
  };
  let efforts = efforts.clone();
  let chip = Button::empty()
    .button()
    .id(format!("model-effort-{}", effort.as_str()))
    .height(Dimension::Px(CHIP_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(stroke, color))
    .rounded(RadiusSize::Sm)
    .describe("selected", if chosen { "true" } else { "false" })
    .child(
      Text::new(effort.as_str())
        .variant(TypographyStyle::Mono)
        .color(if chosen {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextSecondary
        }),
    );
  keyboard_stop(chip, Ring::Outer)
    .cursor(CursorIcon::Pointer)
    .on_click(move |_| {
      efforts.update(|efforts| {
        if chosen {
          efforts.0.retain(|kept| *kept != effort);
        } else {
          efforts.0.push(effort);
        }
      });
    })
    .into()
}
