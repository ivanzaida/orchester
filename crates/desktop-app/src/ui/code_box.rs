//! A long value in Geist Mono with Copy (`Command` `MOPAN` in `Approval card` `gwOaJ`): its label in
//! 11/500 `text_secondary` with a ghost Copy at the row's end, and 6 under them the value in 12
//! `text_primary` on `code_bg` inside a `border` stroke, radius 6, 8 over and under it and 10 at its
//! sides. The value is lurq's `ui::CodeBlock` with no bar (`theme::kit::code_box`): never shortened, it wraps, it
//! can be selected, and past twelve lines it scrolls inside its box. Copy is the kit's, with its word: a Tab stop that
//! says Copied for a moment once the window's clipboard (`ui::clipboard`) holds the value, or that it could not
//! copy. Inline (`Command` `SiCGv`), the box has no caption and Copy is a small icon button inside it at its end, 8
//! over and under and 12 at its sides; its tooltip says Copied. The value's ids are lurq's: the block `<id>`, its
//! code `<id>-code`; the box is `<id>-box` and Copy `<id>-copy`.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
  ui::CodeBlock,
};

use super::{
  button::{ActionButton, ButtonKind},
  clipboard::Clipboard,
  icon::Icon,
  icon_button::IconButton,
};
use crate::theme::{
  ExtraColor, ExtraSpacing,
  kit::code_box::{CODE_BOX, ROW_HEIGHT},
};

/// How long Copy says what happened before it offers to copy again.
const COPY_FEEDBACK: Duration = Duration::from_secs(2);
/// The tallest the value gets before it scrolls: twelve lines of mono 12 at 1.5.
const VALUE_MAX_HEIGHT: f32 = 216.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Copied {
  #[default]
  Not,
  Done,
  Failed,
}

impl DevtoolsInspectable for Copied {}

#[derive(Clone, Debug, PartialEq)]
pub struct CodeBox {
  id: Arc<str>,
  label: Arc<str>,
  value: Arc<str>,
  inline: bool,
}

impl DevtoolsInspectable for CodeBox {}

impl CodeBox {
  /// `value` under `label`; the value's element id is `id` (its code `<id>-code`), Copy's `<id>-copy`.
  pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, value: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      value: value.into(),
      inline: false,
    }
  }

  /// One box with no caption over it, Copy a small icon button inside it at its end (`Command` `SiCGv` in `14`): 8
  /// over and under, 12 at its sides; the label is the box's name for tools.
  pub fn inline(mut self) -> Self {
    self.inline = true;
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let key = format!("code-box-{}", self.id);
    ctx.mount_keyed::<CodeBoxView>(&key, self)
  }
}

struct CodeBoxView {
  copied: Signal<Copied>,
  reset: Timeout,
}

impl Component for CodeBoxView {
  type Props = CodeBox;

  fn create(ctx: &mut Ctx) -> Self {
    let copied = ctx.signal(Copied::Not);
    let back = copied.clone();
    let reset = ctx.create_timeout(COPY_FEEDBACK, move || back.set(Copied::Not));
    Self { copied, reset }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CodeBox>().clone();
    let (icon, key) = match self.copied.get() {
      Copied::Not => (Icon::Copy, "common.copy"),
      Copied::Done => (Icon::Check, "ui.copied"),
      Copied::Failed => (Icon::CircleAlert, "ui.copyFailed"),
    };
    let (copied, reset, value) = (self.copied.clone(), self.reset.clone(), props.value.clone());
    let clipboard = Clipboard::of(ctx);
    let press = move || {
      copied.set(if clipboard.copy(&value) {
        Copied::Done
      } else {
        Copied::Failed
      });
      reset.restart();
    };
    let block = CodeBlock::new(&props.value)
      .id(props.id.clone())
      .key(props.id.clone())
      .label(&props.label)
      .header(false)
      .copy_button(false)
      .framed(false)
      .wrap(true)
      .row_height(ROW_HEIGHT)
      .max_height(VALUE_MAX_HEIGHT)
      .state(CODE_BOX, true);
    let text = Column::new().flex(1.0).child(block);
    let value = Row::new()
      .id(format!("{}-box", props.id))
      .width(Dimension::Pct(100.0))
      .background(ExtraColor::CodeBackground.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(text);
    if props.inline {
      let copy = IconButton::new(format!("{}-copy", props.id), icon, ctx.t(key))
        .small()
        .in_form()
        .on_click(press)
        .render(ctx);
      return Column::new().width(Dimension::Pct(100.0)).child(
        value
          .describe("a11y_name", props.label)
          .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(copy),
      );
    }
    let copy = ActionButton::new(format!("{}-copy", props.id), ButtonKind::Ghost, ctx.t(key))
      .icon(icon)
      .in_form()
      .on_click(press);
    let label = Row::new()
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .child(Text::new(&props.label).variant(TypographyStyle::FieldLabel).flex(1.0))
      .child(copy);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(label)
      .child(value.padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm)))
  }
}
