//! A command or an address with Copy at its end (`Command row` `PEnj5`): the value in Geist Mono 12 on `code_bg`
//! inside a `border` stroke, radius 6, 8 over and under it and 10 at its sides, and a ghost Copy beside it that
//! says Copied, or that it could not copy, for a moment. What it shows may differ from what Copy puts on the
//! clipboard: a command shows its token masked, and Copy copies the full text the caller supplies. A faded row
//! (a server that does not listen) shows its value in `text_muted` and keeps Copy.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
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

/// Puts the row's full text somewhere; `true` once it is there.
pub type Copy = Arc<dyn Fn() -> bool + Send + Sync>;

#[derive(Clone)]
pub struct CommandRow {
  id: Arc<str>,
  shown: Arc<str>,
  copy_label: Arc<str>,
  copy: Copy,
  faded: bool,
}

/// Rows are compared by what they show; Copy goes to the same place.
impl PartialEq for CommandRow {
  fn eq(&self, other: &Self) -> bool {
    (&self.id, &self.shown, &self.copy_label, self.faded) == (&other.id, &other.shown, &other.copy_label, other.faded)
  }
}

impl DevtoolsInspectable for CommandRow {}

impl CommandRow {
  /// `shown` in the box, whose element id is `id` (`<id>-box`, Copy `<id>-copy`); Copy, named `copy_label`,
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
    }
  }

  /// The value in `text_muted`: what it names is not in use now.
  pub fn faded(mut self) -> Self {
    self.faded = true;
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
    let (copied, reset, copy) = (self.copied.clone(), self.reset.clone(), props.copy.clone());
    let button = ActionButton::new(format!("{}-copy", props.id), ButtonKind::Ghost, label)
      .icon(glyph)
      .in_form()
      .on_click(move || {
        copied.set(if copy() { Copied::Done } else { Copied::Failed });
        reset.restart();
      });
    let color = if props.faded {
      PaletteColor::TextMuted
    } else {
      PaletteColor::TextPrimary
    };
    Row::new()
      .id(format!("{}-box", props.id))
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .left(ExtraSpacing::Loose.role())
          .right(ExtraSpacing::Tight.role()),
      )
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(ExtraColor::CodeBackground.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(
        Row::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .padding(Padding::vertical(SpacingSize::Sm))
          .child(
            Text::new(&props.shown)
              .id(props.id.to_string())
              .variant(TypographyStyle::Mono)
              .color(color)
              .width(Dimension::Pct(100.0)),
          ),
      )
      .child(button)
  }
}
