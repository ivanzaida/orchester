//! The chosen step's name, a heading edited in place (`Step name` `eXZOK` in `IsU5u`, `vLoDK` in `gzNi4`, `h8csBx`
//! in `a5RI7`; `Editable title` `BxHzj`). At rest the name in 15/600 `text_primary` is followed, 6 apart, by a 12
//! lucide pencil in `text_muted`, with no fill and no stroke; under the pointer the name takes the `hover` fill, the
//! pencil turns `text_secondary` and the pointer is the text cursor. While it is edited the box takes the 1 px
//! `border_focus` ring and the caret, and the pencil goes; the step column says *Enter to save · Esc to cancel* under
//! it. Enter or leaving the field keeps the name, Esc puts back the name it had when the editing began.
//!
//! lurq lays a single-line input out at a set width and measures no text before layout, so the name is laid out
//! once as an unseen text that sizes the box, and the input over it takes the width that text was laid out at.
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{KeyboardEvent, mouse_event::MouseEvent},
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, TypographyStyle},
  },
  components::{Button, Row, Stack, Text, TextInput},
  core::{ElementRef, Ref, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, HitTestBehavior, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ControlHeight, ExtraColor, ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

/// A width this close to the last one is the same layout.
const SAME_WIDTH: f32 = 0.5;

#[derive(Clone)]
pub struct StepNameProps {
  /// The name as it is typed; its owner takes it into the draft.
  pub value: Signal<String>,
  /// Whether the name is being edited, for the owner to say how it ends.
  pub editing: Signal<bool>,
  /// The field's name for tools.
  pub label: Arc<str>,
  /// The name is not accepted: the ring turns `danger`.
  pub refused: bool,
}

impl PartialEq for StepNameProps {
  fn eq(&self, other: &Self) -> bool {
    self.value.id() == other.value.id()
      && self.editing.id() == other.editing.id()
      && self.label == other.label
      && self.refused == other.refused
  }
}

impl DevtoolsInspectable for StepNameProps {}

pub struct StepName {
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
}

impl Component for StepName {
  type Props = StepNameProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
      input: ElementRef::new(),
      sizer: ElementRef::new(),
      width: ctx.signal(None),
      asked: ctx.signal(0),
      focused_for: ctx.create_ref(0),
      before: ctx.create_ref(String::new()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<StepNameProps>().clone();
    let asked = self.asked.get();
    if asked != self.focused_for.get() {
      self.focused_for.set(asked);
      ctx.focus(&self.input);
    }
    let editing = props.editing.get();
    let hovered = self.hovered.get() && !editing;
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    let mut name = Row::new()
      .id("step-name-box")
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::symmetric(
        ExtraSpacing::Snug.role(),
        ExtraSpacing::Tight.role(),
      ))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .cursor(CursorIcon::Text)
      .describe("editing", if editing { "true" } else { "false" })
      .describe("hovered", if hovered { "true" } else { "false" })
      .describe("state", if props.refused { "invalid" } else { "valid" })
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .child(self.field(ctx, &props));
    if hovered {
      name = name.background(ExtraColor::Hover.role());
    }
    if editing || props.refused {
      let ring = if props.refused {
        PaletteColor::Danger
      } else {
        PaletteColor::BorderFocus
      };
      name = name.border(Border::inside(BorderSize::Sm, ring));
    }
    if editing {
      return name;
    }
    name.child(self.pencil(&props, hovered))
  }
}

impl StepName {
  /// The unseen name that sizes the box, and the input over it as wide as the name was laid out.
  fn field(&self, ctx: &Ctx, props: &StepNameProps) -> Stack {
    let theme = ctx.theme();
    let shown = FormTextRole {
      typography: TypographyStyle::Title,
      color: PaletteColor::TextPrimary,
    }
    .resolve(&theme.typography(), &theme.palette());
    let tracking = shown.letter_spacing;
    let (began, ended) = (props.editing.clone(), props.editing.clone());
    let (before, value) = (self.before.clone(), props.value.clone());
    let (restore, cancelled) = (self.before.clone(), props.value.clone());
    let mut input = TextInput::styled(props.value.clone(), shown)
      .letter_spacing(tracking)
      .caret_color(PaletteColor::TextPrimary)
      .id("step-name")
      .describe("a11y_name", props.label.clone())
      .ref_element(self.input.clone())
      .single_line()
      .on_focus(move || {
        before.set(value.get_untracked());
        began.set(true);
      })
      .on_blur(move || ended.set(false))
      // lurq ends the editing after this, on Enter and on Esc alike.
      .on_key_down(move |event: KeyboardEvent| {
        if event.key == "Escape" && !event.composing {
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
    let name = props.value.get();
    let sizer = Text::new(if name.is_empty() { " " } else { name.as_str() })
      .id("step-name-sizer")
      .ref_element(self.sizer.clone())
      .variant(TypographyStyle::Title)
      .nowrap()
      .opacity(0.0)
      .hit_test(HitTestBehavior::None);
    Stack::new().child(sizer).child(input)
  }

  /// The pencil after the name, which edits it too.
  fn pencil(&self, props: &StepNameProps, hovered: bool) -> Button {
    let asked = self.asked.clone();
    let color = if hovered {
      PaletteColor::TextSecondary
    } else {
      PaletteColor::TextMuted
    };
    Button::empty()
      .id("step-name-edit")
      .focusable(false)
      .cursor(CursorIcon::Text)
      .describe("a11y_name", props.label.clone())
      .on_click(move |event: MouseEvent| {
        event.stop_propagation();
        asked.update(|n| *n += 1);
      })
      .child(icon(Icon::Pencil, IconSize::Sm, color))
  }
}

/// The name of a step that cannot be edited now, a viewed version's (`aGlsL` in `BxHzj`): no pencil, no hover.
pub fn read_only(name: &str) -> Row {
  Row::new()
    .id("step-name-box")
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::symmetric(
      ExtraSpacing::Snug.role(),
      ExtraSpacing::Tight.role(),
    ))
    .align_items(Alignment::Center)
    .child(Text::new(name).id("step-name").variant(TypographyStyle::Title).nowrap())
}
