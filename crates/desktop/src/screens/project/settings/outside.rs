//! How a settings section follows the changes automation makes to its own setting through Orchester's
//! MCP server: it reads the setting again and shows it in place, unless the person has a change of
//! their own under way (being saved, or typed and not saved yet). Then it keeps what the person sees
//! and says, quietly, that the setting changed outside the screen, with *Reload* to show what is
//! stored. The person's own save is made on what is stored, so it keeps the outside change; once it is
//! done the section reads the setting again and the notice goes. A reading that started before the
//! latest change or save is dropped, so it never shows a value older than one already shown.

use lurq::{app::ctx::Ctx, core::Signal, node::Element};

use crate::{
  screens::project::ProjectNav,
  tasks::outside::Outside,
  ui::{button::ActionButton, icon::Icon, notice::Notice},
};

/// Which reading of the setting a section asked for: after how many outside changes of it, *Reload*s
/// and saves of the section's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Reading {
  outside: u64,
  reload: u64,
  saves: u64,
}

impl lurq::app::component::DevtoolsInspectable for Reading {}

#[derive(Clone)]
pub(super) struct OutsideSettings {
  /// The setting the section shows.
  setting: Outside,
  /// The reading last shown.
  applied: Signal<Option<Reading>>,
  /// An outside change came while the person had a change under way: it waits for *Reload*.
  held: Signal<bool>,
  /// *Reload* was pressed: the next reading is shown whatever the person typed.
  forced: Signal<bool>,
  reload: Signal<u64>,
  saves: Signal<u64>,
}

impl OutsideSettings {
  pub(super) fn new(ctx: &mut Ctx, setting: Outside) -> Self {
    Self {
      setting,
      applied: ctx.signal(None),
      held: ctx.signal(false),
      forced: ctx.signal(false),
      reload: ctx.signal(0),
      saves: ctx.signal(0),
    }
  }

  /// The reading the section must show now.
  pub(super) fn reading(&self, ctx: &mut Ctx) -> Reading {
    let outside = ctx
      .use_context::<ProjectNav>()
      .map_or(0, |nav| nav.feed.outside.get().of(self.setting));
    Reading {
      outside,
      reload: self.reload.get(),
      saves: self.saves.get(),
    }
  }

  /// The section's own save is done: it holds the outside change too, so the notice goes, and the
  /// setting is read again.
  pub(super) fn saved(&self) {
    if self.held.get_untracked() {
      self.held.set(false);
    }
    self.saves.update(|saves| *saves += 1);
  }

  /// Whether the section shows `read`, a reading that finished, when `current` is the one it needs
  /// now and `busy` says the person has a change under way. The first reading is always shown.
  pub(super) fn take(&self, read: Reading, current: Reading, busy: bool) -> bool {
    let applied = self.applied.get_untracked();
    if read != current || applied == Some(read) {
      return false;
    }
    let forced = self.forced.get_untracked();
    if applied.is_some() && !forced && (busy || self.held.get_untracked()) {
      let outside_changed = applied.is_some_and(|applied| applied.outside != read.outside);
      if outside_changed && !self.held.get_untracked() {
        self.held.set(true);
      }
      return false;
    }
    if forced {
      self.forced.set(false);
    }
    self.applied.set(Some(read));
    true
  }

  /// The notice while an outside change waits for *Reload*, with *Reload*.
  pub(super) fn notice(&self, ctx: &Ctx, id: &'static str) -> Option<Element> {
    if !self.held.get() {
      return None;
    }
    let (held, forced, reload) = (self.held.clone(), self.forced.clone(), self.reload.clone());
    let button = ActionButton::new(ctx.t("common.reload"))
      .id(format!("{id}-reload"))
      .on_click(move || {
        held.set(false);
        forced.set(true);
        reload.update(|count| *count += 1);
      })
      .render(ctx);
    Some(
      Notice::new(
        Icon::RefreshCw,
        ctx.t("settings.changedOutside.title"),
        ctx.t("settings.changedOutside.detail"),
      )
      .id(id)
      .action(button)
      .into(),
    )
  }
}
