//! A long value in Geist Mono with Copy (`Command` `MOPAN` in `Approval card` `gwOaJ`): its label in
//! 11/500 `text_secondary` with a ghost Copy at the row's end, and 6 under them the value in 12
//! `text_primary` on `code_bg` inside a `border` stroke, radius 6, 8 over and under it and 10 at its
//! sides. The value is never shortened: it wraps, it can be selected, and past twelve lines it scrolls
//! inside its box. Copy is a Tab stop; it says Copied for a moment once the clipboard holds the value,
//! or that it could not copy.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  clipboard::copy_to_clipboard,
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
};
use crate::theme::{ExtraColor, ExtraSpacing};

/// How long Copy says what happened before it offers to copy again.
const COPY_FEEDBACK: Duration = Duration::from_secs(2);
/// The tallest the value gets before it scrolls: twelve lines of mono 12 at 1.5.
const VALUE_MAX_HEIGHT: f32 = 216.0;

/// Where Copy puts the value; `true` once it is there.
#[derive(Clone)]
pub struct Copier(Arc<dyn Fn(&str) -> bool + Send + Sync>);

impl Copier {
  /// The system clipboard.
  pub fn clipboard() -> Self {
    Self(Arc::new(|text| copy_to_clipboard(text)))
  }

  /// Somewhere else, e.g. a gallery that must leave the person's clipboard alone.
  pub fn other(copy: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
    Self(Arc::new(copy))
  }
}

impl PartialEq for Copier {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl std::fmt::Debug for Copier {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("Copier")
  }
}

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
  copier: Copier,
}

impl DevtoolsInspectable for CodeBox {}

impl CodeBox {
  /// `value` under `label`; the value's element id is `id`, Copy's `<id>-copy`.
  pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, value: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      value: value.into(),
      copier: Copier::clipboard(),
    }
  }

  /// Copy puts the value there instead of the clipboard.
  pub fn copy_with(mut self, copier: Copier) -> Self {
    self.copier = copier;
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
    let copier = props.copier.clone();
    let copy = ActionButton::new(format!("{}-copy", props.id), ButtonKind::Ghost, ctx.t(key))
      .icon(icon)
      .in_form()
      .on_click(move || {
        copied.set(if (copier.0)(&value) {
          Copied::Done
        } else {
          Copied::Failed
        });
        reset.restart();
      });
    let label = Row::new()
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .child(Text::new(&props.label).variant(TypographyStyle::FieldLabel).flex(1.0))
      .child(copy);
    let value = Row::new()
      .id(format!("{}-box", props.id))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
      .background(ExtraColor::CodeBackground.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(
        ScrollVertical::new(
          Text::new(&props.value)
            .id(props.id.to_string())
            .variant(TypographyStyle::Mono)
            .selectable(true)
            .width(Dimension::Pct(100.0)),
        )
        .max_height(Dimension::Px(VALUE_MAX_HEIGHT))
        .flex(1.0),
      );
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(label)
      .child(value)
  }
}
