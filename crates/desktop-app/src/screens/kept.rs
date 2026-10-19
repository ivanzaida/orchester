//! What screens keep for as long as the app runs, beyond the screen that shows it: leaving a screen and coming back
//! finds it. Kept by key; an entry left with nothing goes.

use std::{collections::HashMap, hash::Hash, sync::LazyLock};

use parking_lot::Mutex;

/// What is kept under one key, which may hold nothing.
pub(crate) trait Entry: Default {
  fn is_empty(&self) -> bool;
}

impl Entry for String {
  fn is_empty(&self) -> bool {
    self.as_str().is_empty()
  }
}

/// What is kept, by key; made as a `static`.
pub(crate) struct ByKey<K, V>(LazyLock<Mutex<HashMap<K, V>>>);

impl<K: Copy + Eq + Hash, V: Entry> ByKey<K, V> {
  pub(crate) const fn new() -> Self {
    Self(LazyLock::new(Mutex::default))
  }

  /// Changes what is kept under `key`; an entry left with nothing goes.
  pub(crate) fn change<T>(&self, key: K, change: impl FnOnce(&mut V) -> T) -> T {
    let mut all = self.0.lock();
    let kept = all.entry(key).or_default();
    let changed = change(kept);
    if kept.is_empty() {
      all.remove(&key);
    }
    changed
  }

  /// Reads what is kept under `key`: the default when nothing is.
  pub(crate) fn read<T: Default>(&self, key: K, read: impl FnOnce(&V) -> T) -> T {
    self.0.lock().get(&key).map(read).unwrap_or_default()
  }
}
