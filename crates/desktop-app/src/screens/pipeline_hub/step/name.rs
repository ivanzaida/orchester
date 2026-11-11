//! The open step's name in its head, edited in place (`Name` `d6Hm2` in P2b `EGEos`, editing P2e `FHktg`, refused P2g
//! `hYIuX`): the name in the panel's title style with a 12 pencil after it; a press on either puts the caret in it.
//! While it is edited its box takes the 1 px `border_focus` ring and the pencil goes; a name that is not accepted turns
//! the ring `danger`. Enter or leaving the field keeps the name, Esc puts back the name it had when the editing began.
//! The name goes into the panel's edit as it is typed; a refused one is kept as the step's invalid text
//! ([`super::Faulty`]), so the panel does not offer to save until it is right.
//!
//! Nothing moves when the editing begins or ends (D-022): the box keeps its room at all times and stands 8 to the
//! start of where the name is, so the name sits where the head's other lines start; the pencil, *Enter to save · Esc to
//! cancel* and why a name is refused take turns in one place after the box, on its line, rather than under it.
//!
//! lurq lays a single-line input out at a set width and measures no text before layout, so the name is laid out once
//! as an unseen text that sizes the box, and the input over it takes the width that text was laid out at.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{KeyboardEvent, mouse_event::MouseEvent},
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Row, Stack, Text, TextInput},
  core::{ElementRef, Ref, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, HitTestBehavior, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::StepKey;
use orchester_workspace::messages::Message;

use super::{
  Faulty, kept,
  text_field::{Kind, take},
};
use crate::{
  screens::pipeline_hub::edit::Edit,
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, HubText, IconSize},
  ui::{
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    tooltip::Truncated,
  },
};

/// The name as words in the box's room, `id`'s, cut with an ellipsis and whole in a tooltip where it does not fit.
fn as_words(ctx: &mut Ctx, id: &str, name: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .min_width(Dimension::Px(0.0))
    .child(Truncated::new(id.to_owned(), name.to_owned(), HubText::PanelTitle.role()).render(ctx))
}

/// A width this close to the last one is the same layout.
const SAME_WIDTH: f32 = 0.5;

#[derive(Clone)]
pub(super) struct Props {
  pub edit: Edit,
  pub key: StepKey,
  /// The name the edit holds now, which the field starts with.
  pub shown: String,
  /// The field's name for tools.
  pub label: Arc<str>,
  /// A save is under way: the name reads, it is not edited.
  pub locked: bool,
  pub epoch: u64,
  pub faulty: Faulty,
}

/// The props are equal when they say the same: the edit's handle and the panel's list are its own.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.key, &self.shown, &self.label, self.locked) == (&other.key, &other.shown, &other.label, other.locked)
  }
}
impl DevtoolsInspectable for Props {}

/// The name of `props`, mounted anew for each step and after each save or *Cancel*.
pub(super) fn field(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("pipeline-hub-step-name-{}-{}", props.key, props.epoch);
  ctx.mount_keyed::<StepName>(&key, props)
}

struct StepName {
  text: Signal<String>,
  /// Why the name is not accepted.
  error: Signal<Option<Message>>,
  editing: Signal<bool>,
  hovered: Signal<bool>,
  input: ElementRef,
  /// The unseen name, and the width it was laid out at.
  sizer: ElementRef,
  width: Signal<Option<f32>>,
  /// Presses on the pencil asked to edit, and how many of them have focused the input.
  asked: Signal<u32>,
  focused_for: Ref<u32>,
  /// The name when the editing began, which Esc puts back.
  before: Ref<String>,
  /// The panel's edit as of the last draw, which a change goes to.
  edit: Ref<Option<Edit>>,
}

impl Component for StepName {
  type Props = Props;

  /// The field starts with the name the edit holds, or, when a refused name was typed here and the person came back to
  /// the step, with that name and why it is refused.
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let typed = kept(&props.faulty, &props.key, Kind::Name);
    let error = typed.as_deref().and_then(|text| Kind::Name.refused(text));
    let this = Self {
      text: ctx.signal(typed.unwrap_or_else(|| props.shown.clone())),
      error: ctx.signal(error),
      editing: ctx.signal(false),
      hovered: ctx.signal(false),
      input: ElementRef::new(),
      sizer: ElementRef::new(),
      width: ctx.signal(None),
      asked: ctx.signal(0),
      focused_for: ctx.create_ref(0),
      before: ctx.create_ref(String::new()),
      edit: ctx.create_ref(Some(props.edit.clone())),
    };
    let (error, edit) = (this.error.clone(), this.edit.clone());
    let (key, faulty) = (props.key.clone(), props.faulty);
    ctx.watch(&this.text, move |text: &String| {
      error.set(take(text, (&key, Kind::Name), &faulty, edit.get().as_ref()));
    });
    this
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.edit.set(Some(props.edit.clone()));
    let asked = self.asked.get();
    if asked != self.focused_for.get() {
      self.focused_for.set(asked);
      ctx.focus(&self.input);
    }
    let editing = self.editing.get() && !props.locked;
    let error = self.error.get();
    let inset = ctx.theme().spacing_value(SpacingSize::Sm).to_px();
    let after: Option<Element> = match (&error, editing, props.locked) {
      (Some(error), ..) => Some(
        field_error("pipeline-hub-step-name-error", &message_text(ctx, error))
          .width(Dimension::Auto)
          .flex_shrink(0.0)
          .padding(Padding::new().left(ExtraSpacing::Snug.role()))
          .into(),
      ),
      (None, true, _) => Some(
        Text::new(&ctx.t("pipeline_hub.stepEdit.nameHint"))
          .id("pipeline-hub-step-name-hint")
          .variant(HubText::PanelSub.role())
          .padding(Padding::new().left(ExtraSpacing::Snug.role()))
          .nowrap()
          .flex_shrink(0.0)
          .into(),
      ),
      (None, false, locked) => Some(self.pencil(&props, locked).into()),
    };
    Row::new()
      .id("pipeline-hub-step-name-line")
      // As wide as the head's column at most: a long name gives way inside its box, so what follows it stays in view.
      .width(Dimension::Pct(100.0))
      .min_width(Dimension::Px(0.0))
      .offset(-inset, 0.0)
      .align_items(Alignment::Center)
      .child(self.name_box(ctx, &props, editing, error.is_some()))
      .with_children(after)
  }
}

impl StepName {
  /// The box: 28 high, 8 at its sides, radius `md`; the ring while the name is edited or refused, the `hover` fill
  /// under the pointer while it is not.
  fn name_box(&self, ctx: &mut Ctx, props: &Props, editing: bool, refused: bool) -> Row {
    let hovered = self.hovered.get() && !editing && !props.locked;
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    let content: Element = if props.locked {
      as_words(ctx, Kind::Name.name(), &self.text.get()).into()
    } else {
      self.input(ctx, props, editing).into()
    };
    let mut name = Row::new()
      .id("pipeline-hub-step-name-box")
      .min_width(Dimension::Px(0.0))
      .flex_shrink(1.0)
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .cursor(if props.locked {
        CursorIcon::Default
      } else {
        CursorIcon::Text
      })
      .describe("editing", editing.to_string())
      .describe("state", if refused { "invalid" } else { "valid" })
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .child(content);
    // At rest the name is words over the field: a press on them starts the editing, as the pencil does.
    if !editing && !props.locked {
      let asked = self.asked.clone();
      name = name.on_click(move |_: MouseEvent| asked.update(|n| *n += 1));
    }
    if hovered {
      name = name.background(ExtraColor::Hover.role());
    }
    if refused || editing {
      let ring = if refused {
        PaletteColor::Danger
      } else {
        PaletteColor::BorderFocus
      };
      name = name.border(Border::inside(BorderSize::Sm, ring));
    }
    name
  }

  /// The unseen name that sizes the box, and the input over it as wide as the name was laid out. At rest the input is
  /// unseen under the name as words, which a long name ends in an ellipsis with the whole name in its tooltip.
  fn input(&self, ctx: &mut Ctx, props: &Props, editing: bool) -> Stack {
    let theme = ctx.theme();
    let shown = FormTextRole {
      typography: HubText::PanelTitle.role(),
      color: PaletteColor::TextPrimary,
    }
    .resolve(&theme.typography(), &theme.palette());
    let tracking = shown.letter_spacing;
    let (began, ended) = (self.editing.clone(), self.editing.clone());
    let (before, value) = (self.before.clone(), self.text.clone());
    let (restore, cancelled, edited) = (self.before.clone(), self.text.clone(), self.editing.clone());
    let mut input = TextInput::styled(self.text.clone(), shown)
      .letter_spacing(tracking)
      .caret_color(PaletteColor::TextPrimary)
      .id(Kind::Name.name())
      .describe("a11y_name", props.label.clone())
      .ref_element(self.input.clone())
      .single_line()
      .on_focus(move || {
        before.set(value.get_untracked());
        began.set(true);
      })
      .on_blur(move || ended.set(false))
      // lurq ends the editing after this, on Enter and on Esc alike. An Esc reaches the input whether or not it has the
      // focus: only one that ends an editing puts its name back.
      .on_key_down(move |event: KeyboardEvent| {
        if event.key == "Escape" && !event.composing && edited.get_untracked() {
          cancelled.set(restore.get());
        }
      });
    let width = self.width.clone();
    self.sizer.observe_rect(move |rect| {
      let known = width
        .get_untracked()
        .is_some_and(|last| (last - rect.width).abs() < SAME_WIDTH);
      if rect.width > 0.0 && !known {
        width.set(Some(rect.width));
      }
    });
    // A little more than the name, for the caret after its last letter.
    if let Some(width) = self.width.get() {
      input = input.width(Dimension::Px(width + ExtraSpacing::Tight.value()));
    }
    // An empty name keeps a space's width, so the field can still be pressed.
    let name = self.text.get();
    let sizer = Text::new(if name.is_empty() { " " } else { name.as_str() })
      .id("pipeline-hub-step-name-sizer")
      .ref_element(self.sizer.clone())
      .variant(HubText::PanelTitle.role())
      .nowrap()
      .opacity(0.0)
      .hit_test(HitTestBehavior::None);
    let words = (!editing).then(|| as_words(ctx, "pipeline-hub-step-name-shown", &name));
    if !editing {
      input = input.opacity(0.0);
    }
    Stack::new()
      .min_width(Dimension::Px(0.0))
      .flex_shrink(1.0)
      .child(sizer)
      .child(input)
      .with_children(words)
  }

  /// The pencil after the name, which edits it too; faded and not pressable while a save is under way.
  fn pencil(&self, props: &Props, locked: bool) -> Button {
    let color = if self.hovered.get() && !locked {
      PaletteColor::TextSecondary
    } else {
      PaletteColor::TextMuted
    };
    let button = Button::empty()
      .id("pipeline-hub-step-name-edit")
      .focusable(false)
      .flex_shrink(0.0)
      .describe("a11y_name", props.label.clone())
      .child(icon(Icon::Pencil, IconSize::Sm, color));
    if locked {
      return button.opacity(DISABLED_OPACITY).hit_test(HitTestBehavior::None);
    }
    let asked = self.asked.clone();
    button.cursor(CursorIcon::Text).on_click(move |event: MouseEvent| {
      event.stop_propagation();
      asked.update(|n| *n += 1);
    })
  }
}
