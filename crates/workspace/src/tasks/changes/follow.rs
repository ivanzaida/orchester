//! Reading a run's worktree again while an agent works in it, only as long as the page that shows
//! it is open and on screen. The page owns a [`Refresh`] and ticks it from its own interval, which
//! stops while its window tab is offstage; a read that follows it ends once the page, and with it
//! the `Refresh`, is gone, even if nothing cancels the task it runs on.

use std::{
  future::Future,
  sync::{Arc, Weak},
};

use tokio::sync::watch;

/// The ticks that wake the worktree reads of one page.
#[derive(Debug)]
pub struct Refresh(Arc<watch::Sender<u64>>);

impl Default for Refresh {
  fn default() -> Self {
    Self(Arc::new(watch::channel(0).0))
  }
}

impl Refresh {
  /// Wakes the reads that follow this refresh; for the page's interval, which may outlive it.
  pub fn ticker(&self) -> impl Fn() + Send + Sync + 'static {
    let ticks = Arc::downgrade(&self.0);
    move || {
      if let Some(ticks) = ticks.upgrade() {
        ticks.send_modify(|tick| *tick = tick.wrapping_add(1));
      }
    }
  }

  /// What a read needs to follow this refresh without keeping it alive.
  pub fn follower(&self) -> Follower {
    Follower(Arc::downgrade(&self.0))
  }
}

/// A [`Refresh`] as a read follows it: it ends with the refresh.
#[derive(Clone, Debug)]
pub struct Follower(Weak<watch::Sender<u64>>);

impl Follower {
  /// Reads with `read` and passes each result that differs from the one before it to `emit`;
  /// while `live`, reads again on each tick of the refresh. Ends after the first read when not
  /// `live`, once the refresh is gone, or once `emit` refuses a result.
  pub async fn follow<T, Read, Fut>(self, live: bool, mut read: Read, mut emit: impl FnMut(T) -> bool)
  where
    T: Clone + PartialEq,
    Read: FnMut() -> Fut,
    Fut: Future<Output = T>,
  {
    let Some(mut ticks) = self.0.upgrade().map(|ticks| ticks.subscribe()) else {
      return;
    };
    let mut last = None;
    loop {
      let value = read().await;
      if last.as_ref() != Some(&value) {
        if !emit(value.clone()) {
          return;
        }
        last = Some(value);
      }
      if !live || ticks.changed().await.is_err() {
        return;
      }
    }
  }
}
