//! What the Conversation tab hands the parts it mounts and changes as it renders (the composer's task and run, the
//! chat's timeline), provided once from its `create` as a signal it sets when the value changes (ORC-71). lurq
//! renders again every component under one that provides a context while it renders: provided in each render, they
//! made every render of the tab render again every row the chat had built, twice as a long run's chat opened. Read
//! through the signal, a part renders again when the value it reads changed.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};

/// A value the tab shares with what it mounts.
#[derive(Clone)]
pub(super) struct Shared<T: DevtoolsInspectable + 'static>(Signal<Option<T>>);

impl<T> Shared<T>
where
  T: Clone + PartialEq + Send + Sync + DevtoolsInspectable + 'static,
{
  /// Provided to everything the tab mounts, from its `create` on; no value until the first [`Self::set`]. Not a
  /// signal of the tab's own: setting it renders again only those that read it.
  pub(super) fn provide(ctx: &mut Ctx) -> Self {
    let shared = Self(Signal::new(None));
    ctx.provide(shared.clone());
    shared
  }

  /// Shares `value` from now on, when it is not the value shared already.
  pub(super) fn set(&self, value: T) {
    if self.0.get_untracked().as_ref() != Some(&value) {
      self.0.set(Some(value));
    }
  }

  /// The value the tab shares, read in a render of a part it mounted, which renders again when the value changes;
  /// `None` outside the tab or before it shared one.
  pub(super) fn of(ctx: &mut Ctx) -> Option<T> {
    ctx.use_context::<Self>()?.0.get()
  }
}
