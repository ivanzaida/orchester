//! The pipeline's name edited in place (P11 `nrTt0`: *Rename* makes the title editable): the title's text becomes a
//! field in the same 16/600, in a `border_focus` ring (`danger` once the name was refused), with the reason under it.
//! Enter saves, Esc and leaving the field keep the name the pipeline had. A name another pipeline has is refused with
//! the reason and the field stays open.

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    events::KeyboardEvent,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text, TextInput},
  core::{ElementRef, Signal},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::messages::Message;

use crate::{
  theme::{ExtraSpacing, HubText},
  ui::{field::field_error, message::message_text},
};

/// Whether the name is being edited, and what is typed.
#[derive(Clone)]
pub(super) struct Editing {
  pub open: Signal<bool>,
  name: Signal<String>,
  input: ElementRef,
  /// The field opened and wants the keys.
  focus: Signal<bool>,
  /// Enter was pressed since the field opened, so what the save says is about the name typed.
  asked: Signal<bool>,
}

impl Editing {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      name: ctx.signal(String::new()),
      input: ElementRef::new(),
      focus: ctx.signal(false),
      asked: ctx.signal(false),
    }
  }

  /// Opens the field on `name`, the name the pipeline has.
  pub(super) fn begin(&self, name: &str) {
    self.name.set(name.to_owned());
    self.asked.set(false);
    self.open.set(true);
    self.focus.set(true);
  }

  fn end(&self) {
    self.asked.set(false);
    self.open.set(false);
  }
}

/// The field, with what the last Enter was refused for under it; Enter asks `rename` to save the name typed.
pub(super) fn field(ctx: &mut Ctx, editing: &Editing, rename: &FutureAction<String, (), Message>) -> Element {
  if editing.focus.get() {
    ctx.focus(&editing.input);
    editing.focus.set(false);
  }
  let state = rename.state().get();
  let pending = state.is_pending();
  let refusal = state
    .error
    .filter(|_| !pending && editing.asked.get())
    .map(|error| message_text(ctx, &error));
  let theme = ctx.theme();
  let typed = FormTextRole {
    typography: HubText::PanelTitle.role(),
    color: PaletteColor::TextPrimary,
  }
  .resolve(&theme.typography(), &theme.palette());
  let (save, give_up, leave) = (editing.clone(), editing.clone(), editing.clone());
  let (action, typed_name) = (rename.clone(), editing.name.clone());
  let input = TextInput::styled(editing.name.clone(), typed)
    .id("pipeline-hub-rename")
    .describe("a11y_name", ctx.t("pipeline_hub.rename.label"))
    .describe("state", if refusal.is_some() { "invalid" } else { "valid" })
    .width(Dimension::Pct(100.0))
    .caret_color(PaletteColor::TextPrimary)
    .ref_element(editing.input.clone())
    .single_line()
    .on_blur(move || leave.end())
    .on_key_down(move |event: KeyboardEvent| match event.key.as_str() {
      "Enter" if event.text_input_focused && !event.composing => {
        event.prevent_default();
        if !action.state().get_untracked().is_pending() {
          save.asked.set(true);
          action.run(typed_name.get_untracked());
        }
      }
      "Escape" if !event.composing => {
        event.prevent_default();
        give_up.end();
      }
      _ => {}
    });
  let ring = if refusal.is_some() {
    PaletteColor::Danger
  } else {
    PaletteColor::BorderFocus
  };
  let boxed = Row::new()
    .id("pipeline-hub-rename-box")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Snug.role(),
      ExtraSpacing::Tight.role(),
    ))
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, ring))
    .child(input);
  Column::new()
    .id("pipeline-hub-rename-field")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(boxed)
    .with_children(refusal.map(|reason| {
      field_error(
        "pipeline-hub-rename-error",
        &ctx.t_args("pipeline_hub.rename.failed", [("reason", &*reason)]),
      )
    }))
    .child(
      Text::new(&ctx.t("pipeline_hub.rename.hint"))
        .id("pipeline-hub-rename-hint")
        .variant(HubText::PanelHint.role())
        .nowrap(),
    )
    .into()
}
