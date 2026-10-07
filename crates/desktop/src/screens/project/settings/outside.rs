//! How a settings section that saves each change at once follows the changes automation makes to the
//! same settings through Orchester's MCP server: it reads them again and shows them in place, unless a
//! change of the person's is being saved. Then it keeps what the person sees and says, quietly, that
//! the settings changed outside the screen, with *Reload* to show what is stored.

use lurq::{app::ctx::Ctx, core::Signal, node::Element};

use crate::{
  screens::project::ProjectNav,
  ui::{button::ActionButton, icon::Icon, notice::Notice},
};

#[derive(Clone)]
pub(super) struct OutsideSettings {
  /// The reading last shown.
  applied: Signal<Option<u64>>,
  /// A reading after an outside change waits for *Reload*: it came while the person's change saved.
  held: Signal<bool>,
  /// Counts *Reload*.
  reload: Signal<u64>,
}

impl OutsideSettings {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      applied: ctx.signal(None),
      held: ctx.signal(false),
      reload: ctx.signal(0),
    }
  }

  /// Which reading of the settings the section shows: a new one after each outside change and each
  /// *Reload*.
  pub(super) fn reading(&self, ctx: &mut Ctx) -> u64 {
    let outside = ctx
      .use_context::<ProjectNav>()
      .map_or(0, |nav| nav.feed.outside.get().settings);
    outside + self.reload.get()
  }

  /// Whether the section shows `reading`, which it has read: the first one always; a later one unless
  /// the person's change is `saving` or an earlier one waits for *Reload*.
  pub(super) fn take(&self, reading: u64, saving: bool) -> bool {
    let applied = self.applied.get_untracked();
    if applied == Some(reading) {
      return false;
    }
    if applied.is_some() && (saving || self.held.get_untracked()) {
      if !self.held.get_untracked() {
        self.held.set(true);
      }
      return false;
    }
    self.applied.set(Some(reading));
    true
  }

  /// The notice while a reading waits for *Reload*, with *Reload*.
  pub(super) fn notice(&self, ctx: &Ctx, id: &'static str) -> Option<Element> {
    if !self.held.get() {
      return None;
    }
    let (held, reload) = (self.held.clone(), self.reload.clone());
    let button = ActionButton::new(ctx.t("common.reload"))
      .id(format!("{id}-reload"))
      .on_click(move || {
        held.set(false);
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
