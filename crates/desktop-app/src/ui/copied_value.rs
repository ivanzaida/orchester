//! A path or an address at a settings row's end with Copy after it, 8 apart (`Value` `KpzaD` in `01 · General`,
//! `Address` `Z2NAcC` in `04 · Automation`): the value in Geist Mono 12 `text_secondary`, cut with an ellipsis when it
//! is too long, and a small Copy. For a moment after a press, Copy's glyph and tooltip say it was copied, or that it
//! could not copy, and the same words show after it, as the command row's Copy says them. What Copy puts on the
//! clipboard may differ from what shows. An unavailable value (a server that does not listen) has its Copy faded.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{icon::Icon, icon_button::IconButton, setting_rows::mono_value};
use crate::theme::ExtraTypography;

/// How long Copy says what happened before it offers to copy again.
const COPY_FEEDBACK: Duration = Duration::from_secs(2);

/// Puts the full value on the clipboard; `true` once it is there.
pub type Copy = Arc<dyn Fn() -> bool + Send + Sync>;

#[derive(Clone)]
pub struct CopiedValue {
  id: Arc<str>,
  shown: Arc<str>,
  label: Arc<str>,
  copy: Copy,
  available: bool,
}

/// Values are compared by what they show; Copy goes to the same place.
impl PartialEq for CopiedValue {
  fn eq(&self, other: &Self) -> bool {
    (&self.id, &self.shown, &self.label, self.available) == (&other.id, &other.shown, &other.label, other.available)
  }
}

impl DevtoolsInspectable for CopiedValue {}

impl CopiedValue {
  /// `shown` (its element id `id`, the pair `<id>-value`, Copy `<id>-copy`, what came of it `<id>-copied`); Copy,
  /// named `label`, runs `copy`.
  pub fn new(
    id: impl Into<Arc<str>>,
    shown: impl Into<Arc<str>>,
    label: impl Into<Arc<str>>,
    copy: impl Fn() -> bool + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      shown: shown.into(),
      label: label.into(),
      copy: Arc::new(copy),
      available: true,
    }
  }

  /// Copy faded: what the value names is not in use now.
  pub fn unavailable(mut self) -> Self {
    self.available = false;
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let key = format!("copied-value-{}", self.id);
    ctx.mount_keyed::<CopiedValueView>(&key, self)
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

struct CopiedValueView {
  copied: Signal<Copied>,
  reset: Timeout,
}

impl Component for CopiedValueView {
  type Props = CopiedValue;

  fn create(ctx: &mut Ctx) -> Self {
    let copied = ctx.signal(Copied::Not);
    let back = copied.clone();
    let reset = ctx.create_timeout(COPY_FEEDBACK, move || back.set(Copied::Not));
    Self { copied, reset }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CopiedValue>().clone();
    let copied = if props.available {
      self.copied.get()
    } else {
      Copied::Not
    };
    let (glyph, label, said) = match copied {
      Copied::Not => (Icon::Copy, props.label.to_string(), None),
      Copied::Done => {
        let words = ctx.t("ui.copied").to_string();
        (Icon::Check, words.clone(), Some((words, PaletteColor::TextMuted)))
      }
      Copied::Failed => {
        let words = ctx.t("ui.copyFailed").to_string();
        (Icon::CircleAlert, words.clone(), Some((words, PaletteColor::Danger)))
      }
    };
    let button = IconButton::new(format!("{}-copy", props.id), glyph, label).small();
    let button = if props.available {
      let (copied, reset, copy) = (self.copied.clone(), self.reset.clone(), props.copy.clone());
      button.on_click(move || {
        copied.set(if copy() { Copied::Done } else { Copied::Failed });
        reset.restart();
      })
    } else {
      button
    };
    let said = said.map(|(words, color)| {
      Text::new(&words)
        .id(format!("{}-copied", props.id))
        .variant(ExtraTypography::Note.role())
        .color(color)
        .nowrap()
    });
    Row::new()
      .id(format!("{}-value", props.id))
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(mono_value(&props.id, &props.shown))
      .child(button.render(ctx))
      .with_children(said)
  }
}
