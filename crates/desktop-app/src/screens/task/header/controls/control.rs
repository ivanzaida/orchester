//! A control of the task page's header, as data: what it says and offers. The header draws the one primary action
//! as a button and every other as a row of *More actions* (`JapSp`), so a control reads the same either way: under
//! way, waiting for something else, or unavailable with why.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};

use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  menu::MenuItem,
  pressable::Press,
};

/// Why a control cannot be used now: in a few words under the header's primary action, and in full.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reason {
  pub short: Arc<str>,
  pub full: Arc<str>,
}

/// What a control offers now.
#[derive(Clone)]
pub enum Offer {
  /// A press asks for it.
  Ready(Press),
  /// Its own request is under way.
  Pending,
  /// It waits for something else under way: another request, or what it opened.
  Disabled,
  /// It applies but cannot be used now, for this reason.
  Unavailable(Reason),
}

/// A control: its element id, words, glyph and look as a button, and what it offers.
#[derive(Clone)]
pub struct Control {
  pub id: Arc<str>,
  pub label: Arc<str>,
  pub icon: Icon,
  pub kind: ButtonKind,
  pub offer: Offer,
  /// The keys that do the same, which its menu row shows.
  pub shortcut: Option<Arc<str>>,
}

impl Control {
  /// The control `id` saying the words of the key `label` with `glyph`, as a button of `kind`, offering `offer`.
  pub fn new(ctx: &Ctx, (id, label, glyph): (&str, &str, Icon), kind: ButtonKind, offer: Offer) -> Self {
    Self {
      id: Arc::from(id),
      label: ctx.t(label),
      icon: glyph,
      kind,
      offer,
      shortcut: None,
    }
  }

  pub const fn reason(&self) -> Option<&Reason> {
    match &self.offer {
      Offer::Unavailable(reason) => Some(reason),
      Offer::Ready(_) | Offer::Pending | Offer::Disabled => None,
    }
  }

  /// The control as a button.
  pub fn button(&self) -> ActionButton {
    let button = ActionButton::new(self.id.clone(), self.kind, self.label.clone()).icon(self.icon);
    match &self.offer {
      Offer::Ready(press) => button.press(press.clone()),
      Offer::Pending => button.loading(),
      Offer::Disabled => button.disabled(),
      Offer::Unavailable(reason) => button.unavailable(reason.full.clone()),
    }
  }

  /// The control as a row of the menu `open` shows, in `danger` when it destroys; one under way or waiting says so
  /// in place of why.
  pub fn item(&self, ctx: &mut Ctx, open: &Signal<bool>) -> Element {
    let (id, label) = (self.id.clone(), self.label.clone());
    let item = match &self.offer {
      Offer::Ready(Press::Run(run)) => {
        let run = run.clone();
        MenuItem::new(id, label, move || run())
      }
      Offer::Ready(Press::Open(to)) => MenuItem::opens(id, label, to.clone()),
      Offer::Pending => MenuItem::unavailable(id, label, ctx.t("task.more.underWay")).without_lock(),
      Offer::Disabled => MenuItem::unavailable(id, label, ctx.t("task.more.waits")).without_lock(),
      Offer::Unavailable(reason) => MenuItem::unavailable(id, label, reason.full.clone()),
    };
    let mut item = item.icon(self.icon);
    if let Some(keys) = &self.shortcut {
      item = item.shortcut(keys.clone());
    }
    let danger = matches!(self.kind, ButtonKind::Danger | ButtonKind::DangerGhost);
    if danger { item.danger() } else { item }.render(ctx, open)
  }
}
