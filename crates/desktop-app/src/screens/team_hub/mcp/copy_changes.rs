//! *Copy my changes* (`Copy my changes` in A14 `gTLx6`): the draft of a person who may no longer publish it, as text on
//! the clipboard, so what they typed is not lost with the page's controls. For a moment after a press the button says
//! whether it copied.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
  },
  core::Signal,
  node::Element,
};

use crate::ui::{
  button::{ActionButton, ButtonKind},
  clipboard::Clipboard,
  icon::Icon,
};

/// How long the button says what happened before it offers to copy again.
const FEEDBACK: Duration = Duration::from_secs(2);

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub text: Arc<str>,
}
impl DevtoolsInspectable for Props {}

pub(super) fn button(ctx: &mut Ctx, text: String) -> Element {
  ctx.mount_keyed::<CopyChanges>("team-hub-edit-copy", Props { text: text.into() })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Copied {
  #[default]
  Not,
  Done,
  Failed,
}
impl DevtoolsInspectable for Copied {}

struct CopyChanges {
  copied: Signal<Copied>,
  reset: Timeout,
  clipboard: Clipboard,
}

impl Component for CopyChanges {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let copied = ctx.signal(Copied::Not);
    let back = copied.clone();
    let reset = ctx.create_timeout(FEEDBACK, move || back.set(Copied::Not));
    Self {
      copied,
      reset,
      clipboard: Clipboard::of(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let text = ctx.props::<Props>().text.clone();
    let (key, glyph) = match self.copied.get() {
      Copied::Not => ("team_hub.edit.copyChanges", Icon::Copy),
      Copied::Done => ("team_hub.edit.copied", Icon::Check),
      Copied::Failed => ("team_hub.edit.copyFailed", Icon::TriangleAlert),
    };
    let (copied, reset, clipboard) = (self.copied.clone(), self.reset.clone(), self.clipboard.clone());
    ActionButton::new("team-hub-edit-copy", ButtonKind::Secondary, ctx.t(key))
      .icon(glyph)
      .on_click(move || {
        copied.set(if clipboard.copy(&text) {
          Copied::Done
        } else {
          Copied::Failed
        });
        reset.restart();
      })
  }
}
