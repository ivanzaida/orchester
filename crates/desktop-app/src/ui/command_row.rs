//! A command or an address with Copy at its end (`Command row` `PEnj5`): the value in Geist Mono 11.5 at 1.4 on
//! `code_bg` inside a `border` stroke, radius 6, 6 over and under it, 10 before it and 4 after Copy, and a 24-high
//! ghost Copy beside it that says Copied, or that it could not copy, for a moment. A command breaks into lines only
//! at its spaces, never inside a word such as `--header`; its own line breaks are kept. What it shows may differ from
//! what Copy puts on the clipboard: a command shows its token masked, and Copy copies the full text the caller
//! supplies. A faded row (a server that does not listen) shows its value in `text_muted`, and its Copy is unavailable.
//! A one-line row (a path) ends in an ellipsis when it is too long.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
};
use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography};

/// How long Copy says what happened before it offers to copy again.
const COPY_FEEDBACK: Duration = Duration::from_secs(2);

/// Puts the row's full text somewhere; `true` once it is there.
pub type Copy = Arc<dyn Fn() -> bool + Send + Sync>;

#[derive(Clone)]
pub struct CommandRow {
  id: Arc<str>,
  shown: Arc<str>,
  copy_label: Arc<str>,
  copy: Copy,
  faded: bool,
  one_line: bool,
}

/// Rows are compared by what they show; Copy goes to the same place.
impl PartialEq for CommandRow {
  fn eq(&self, other: &Self) -> bool {
    (&self.id, &self.shown, &self.copy_label, self.faded, self.one_line)
      == (&other.id, &other.shown, &other.copy_label, other.faded, other.one_line)
  }
}

impl DevtoolsInspectable for CommandRow {}

impl CommandRow {
  /// `shown` in the box (its element id `id`, the box `<id>-box`, Copy `<id>-copy`); Copy, named `copy_label`,
  /// runs `copy`.
  pub fn new(
    id: impl Into<Arc<str>>,
    shown: impl Into<Arc<str>>,
    copy_label: impl Into<Arc<str>>,
    copy: impl Fn() -> bool + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      shown: shown.into(),
      copy_label: copy_label.into(),
      copy: Arc::new(copy),
      faded: false,
      one_line: false,
    }
  }

  /// The value in `text_muted` and Copy unavailable: what it names is not in use now.
  pub fn faded(mut self) -> Self {
    self.faded = true;
    self
  }

  /// The value on one line, its end cut with an ellipsis when it is too long: a path.
  pub fn one_line(mut self) -> Self {
    self.one_line = true;
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let key = format!("command-row-{}", self.id);
    ctx.mount_keyed::<CommandRowView>(&key, self)
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

struct CommandRowView {
  copied: Signal<Copied>,
  reset: Timeout,
}

impl Component for CommandRowView {
  type Props = CommandRow;

  fn create(ctx: &mut Ctx) -> Self {
    let copied = ctx.signal(Copied::Not);
    let back = copied.clone();
    let reset = ctx.create_timeout(COPY_FEEDBACK, move || back.set(Copied::Not));
    Self { copied, reset }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CommandRow>().clone();
    let (glyph, label) = match self.copied.get() {
      Copied::Not => (Icon::Copy, props.copy_label.to_string()),
      Copied::Done => (Icon::Check, ctx.t("ui.copied").to_string()),
      Copied::Failed => (Icon::CircleAlert, ctx.t("ui.copyFailed").to_string()),
    };
    let button = ActionButton::new(format!("{}-copy", props.id), ButtonKind::Ghost, label)
      .icon(glyph)
      .small()
      .in_form();
    let button = if props.faded {
      button.disabled()
    } else {
      let (copied, reset, copy) = (self.copied.clone(), self.reset.clone(), props.copy.clone());
      button.on_click(move || {
        copied.set(if copy() { Copied::Done } else { Copied::Failed });
        reset.restart();
      })
    };
    let color = if props.faded {
      PaletteColor::TextMuted
    } else {
      PaletteColor::TextPrimary
    };
    let value = if props.one_line {
      Text::new(&props.shown)
        .variant(ExtraTypography::MonoKey.role())
        .color(color)
        .width(Dimension::Px(0.0))
        .flex(1.0)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .into()
    } else {
      words(&props.shown, &color)
    };
    Row::new()
      .id(format!("{}-box", props.id))
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Snug.role())
          .bottom(ExtraSpacing::Snug.role())
          .left(ExtraSpacing::Loose.role())
          .right(SpacingSize::Xs),
      )
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(ExtraColor::CodeBackground.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(
        Row::new()
          .id(props.id.to_string())
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .child(value),
      )
      .child(button)
  }
}

/// `text` line by line, each line laid out word by word so it breaks only at its spaces; a word keeps the space
/// after it, so the words read as the text itself.
fn words(text: &str, color: &PaletteColor) -> Element {
  let lines = text.split('\n').map(|line| {
    let count = line.split(' ').count();
    Row::new()
      .width(Dimension::Pct(100.0))
      .wrap()
      .with_children(line.split(' ').enumerate().map(|(index, word)| {
        let word = if index + 1 < count {
          format!("{word} ")
        } else {
          word.to_owned()
        };
        Text::new(&word)
          .variant(ExtraTypography::MonoKey.role())
          .color(color.clone())
          .nowrap()
      }))
  });
  Column::new().width(Dimension::Pct(100.0)).with_children(lines).into()
}
