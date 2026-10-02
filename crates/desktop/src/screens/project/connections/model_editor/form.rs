//! The row being edited (`n5RPYj` in 04.H6 `zHowy`): a highlighted panel headed *Edit model* with the
//! model's source, the model id at full width with its hint and, under it, why the id cannot be saved;
//! the optional context window and max output side by side; the effort chips with their hint; and
//! Remove model, Cancel and Save model. The id of a model something uses stays (the user's decision of
//! 2026-10-02): changing it is refused under the field.

use std::num::NonZeroU32;

use lurq::{
  app::{
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

use super::{super::model_text, CHIP_HEIGHT, Editing, Efforts, ModelEditor, ModelEditorProps, ModelRows, remove};
use crate::{
  connections::model_uses::{ModelUse, UsedKeys, keep_used},
  messages::Message,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{field_error, text_input, wrapping_field_error},
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{Ring, keyboard_stop},
  },
};

/// What the open row says when it would change the id of a model something uses.
const ID_KEYS: UsedKeys = UsedKeys {
  profile: "connections.models.idInUse",
  step: "connections.models.idInUseByStep",
};

/// Height of the editor's buttons (`xowZ3`).
const EDITOR_BUTTON_HEIGHT: f32 = 32.0;

/// Why the open row cannot be saved, by the field it is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum EditError {
  /// The id is not valid, listed already, or would leave something without its model.
  Id(Message),
  /// A token count is not a whole number above zero.
  Limits(Message),
}

impl ModelEditor {
  pub(super) fn editor(&self, ctx: &Ctx, props: &ModelEditorProps, models: &[NativeModel]) -> Element {
    let editing = self.editing.get();
    let model = self.model(models, &editing, &props.uses);
    let (id_error, limits_error) = match model.as_ref().err() {
      Some(Some(EditError::Id(error))) => (Some(message_text(ctx, error)), None),
      Some(Some(EditError::Limits(error))) => (None, Some(message_text(ctx, error))),
      _ => (None, None),
    };
    let before = match &editing {
      Editing::Model(id) => models.iter().find(|model| model.id() == id),
      Editing::New | Editing::None => None,
    };
    let id = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(label(ctx, "connections.models.id"))
      .child(text_input(
        ctx,
        "model-id",
        self.id.clone(),
        id_error.is_some(),
        ExtraTypography::MonoField.role(),
      ))
      .child(hint(ctx, "connections.models.idHint"))
      .with_children(id_error.map(|error| {
        Column::new()
          .id("model-id-error")
          .width(Dimension::Pct(100.0))
          .child(wrapping_field_error(&error))
      }));
    let limits = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Step.role())
      .child(self.limit(ctx, "model-context", "connections.models.contextWindow", &self.context))
      .child(self.limit(
        ctx,
        "model-max-output",
        "connections.models.maxOutputLabel",
        &self.max_output,
      ));
    Column::new()
      .id("model-editor")
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Md)
      .background(PaletteColor::SurfacePanel)
      .border(Border::inside(BorderSize::Sm, PaletteColor::TextPrimary))
      .rounded(ExtraRadius::Notice.role())
      .overflow_visible()
      .child(heading(ctx, before))
      .child(id)
      .child(limits)
      .with_children(limits_error.map(|error| field_error(&error)))
      .child(self.efforts_field(ctx))
      .child(self.editor_actions(ctx, props, &editing, model.ok()))
      .into()
  }

  fn limit(&self, ctx: &Ctx, id: &'static str, key: &str, value: &Signal<String>) -> Element {
    Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Sm)
      .child(label(ctx, key))
      .child(text_input(
        ctx,
        id,
        value.clone(),
        false,
        ExtraTypography::MonoField.role(),
      ))
      .into()
  }

  /// *Efforts* (`fZGVO`): a chip per effort, the chosen ones checked, and what they mean.
  fn efforts_field(&self, ctx: &Ctx) -> Element {
    let chosen = self.efforts.get().0;
    let chips = Row::new()
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .with_children(Effort::ALL.map(|effort| effort_chip(&self.efforts, effort, chosen.contains(&effort))));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .child(label(ctx, "connections.models.efforts"))
      .child(chips)
      .child(hint(ctx, "connections.models.effortsHint"))
      .into()
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
        editor_button(ctx, "model-remove", "connections.models.removeModel", Icon::Trash)
          .on_click({
            // The row being edited is gone with its model, so nothing is open for editing.
            let (remove, editing) = (remove(&props.models, id.clone()), self.editing.clone());
            move || {
              remove();
              editing.set(Editing::None);
            }
          })
          .render(ctx),
      ),
      _ => None,
    };
    let cancel = self.editing.clone();
    let save = editor_button(ctx, "model-save", "connections.models.save", Icon::Check).primary();
    let save = match model {
      Some(model) => save.on_click(save_model(&props.models, &self.editing, editing, model)),
      None => save.on_click(|| {}).enabled(false),
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .overflow_visible()
      .with_children(remove_model)
      .child(Spacer::new().flex(1.0))
      .child(
        editor_button(ctx, "model-cancel", "common.cancel", Icon::X)
          .on_click(move || cancel.set(Editing::None))
          .render(ctx),
      )
      .child(save.render(ctx))
      .into()
  }

  /// The model the open row would save, or why it cannot be saved: `None` while the id is still
  /// empty, which needs no explanation. An edit that leaves the model as it was keeps its source;
  /// any change makes it a model added by hand. The id of a model something uses stays.
  fn model(
    &self,
    models: &[NativeModel],
    editing: &Editing,
    uses: &[ModelUse],
  ) -> Result<NativeModel, Option<EditError>> {
    let entered = self.id.get();
    let entered = entered.trim();
    if entered.is_empty() {
      return Err(None);
    }
    let invalid = |key| Some(EditError::Id(Message::new(key)));
    let id = ModelId::new(entered).map_err(|_| invalid("connections.models.invalidId"))?;
    let listed = models.iter().any(|model| model.id() == &id);
    if listed && *editing != Editing::Model(id.clone()) {
      let listed = Message::new("connections.models.listedAlready").with("model", id.as_str().to_owned());
      return Err(Some(EditError::Id(listed)));
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
    keep_used(&after, uses, ID_KEYS).map_err(|error| Some(EditError::Id(error)))?;
    Ok(model)
  }
}

/// *Edit model* with where the model comes from, or *New model* (`SAxbX`).
fn heading(ctx: &Ctx, before: Option<&NativeModel>) -> Element {
  let (title, source) = match before {
    Some(model) => ("connections.models.editTitle", model_text::source(ctx, model.source())),
    None => (
      "connections.models.newTitle",
      model_text::source(ctx, ModelSource::Manual),
    ),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t(title)).variant(TypographyStyle::Label))
    .child(
      Text::new(&source)
        .variant(ExtraTypography::Hint.role())
        .color(PaletteColor::TextMuted),
    )
    .into()
}

fn label(ctx: &Ctx, key: &str) -> Element {
  Text::new(&ctx.t(key)).variant(TypographyStyle::Caption).into()
}

fn hint(ctx: &Ctx, key: &str) -> Element {
  Text::new(&ctx.t(key)).variant(ExtraTypography::Hint.role()).into()
}

fn editor_button(ctx: &Ctx, id: &'static str, key: &str, glyph: Icon) -> ActionButton {
  ActionButton::new(ctx.t(key))
    .icon(glyph)
    .compact()
    .id(id)
    .in_form()
    .height(Dimension::Px(EDITOR_BUTTON_HEIGHT))
}

/// Puts `model` in place of the row being edited, or adds it, and closes the editor.
fn save_model(
  models: &Signal<ModelRows>,
  open: &Signal<Editing>,
  editing: &Editing,
  model: NativeModel,
) -> impl Fn() + Send + Sync + 'static {
  let (models, open, target) = (models.clone(), open.clone(), editing.clone());
  move || {
    models.update(|rows| match &target {
      Editing::Model(id) => {
        if let Some(row) = rows.0.iter_mut().find(|row| row.id() == id) {
          *row = model.clone();
        }
      }
      Editing::New | Editing::None => rows.0.push(model.clone()),
    });
    open.set(Editing::None);
  }
}

/// An optional token count as typed: empty is unknown, else a whole number above zero.
fn tokens(text: &str) -> Result<Option<NonZeroU32>, Option<EditError>> {
  let text = text.trim();
  if text.is_empty() {
    return Ok(None);
  }
  text
    .parse::<NonZeroU32>()
    .map(Some)
    .map_err(|_| Some(EditError::Limits(Message::new("connections.models.invalidTokens"))))
}

/// An effort the model takes or not (`TkTBZ`, `iHrhY`): a chosen one checked on the hover fill and
/// outlined in the text color, another with a plus on the raised fill.
fn effort_chip(efforts: &Signal<Efforts>, effort: Effort, chosen: bool) -> Element {
  let (glyph, glyph_color, text_color, fill, stroke) = if chosen {
    (
      Icon::Check,
      PaletteColor::TextPrimary,
      PaletteColor::TextPrimary,
      PaletteColor::AccentMuted,
      PaletteColor::TextPrimary,
    )
  } else {
    (
      Icon::Plus,
      PaletteColor::TextMuted,
      PaletteColor::TextSecondary,
      PaletteColor::SurfaceRaised,
      PaletteColor::Border,
    )
  };
  let efforts = efforts.clone();
  let chip = Button::empty()
    .button()
    .id(format!("model-effort-{}", effort.as_str()))
    .height(Dimension::Px(CHIP_HEIGHT))
    .padding_horizontal(ExtraSpacing::Inline.role())
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .background(fill)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Sm)
    .describe("selected", if chosen { "true" } else { "false" })
    .child(icon(glyph, IconSize::Reason, glyph_color))
    .child(
      Text::new(effort.as_str())
        .variant(ExtraTypography::MonoValue.role())
        .color(text_color),
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
