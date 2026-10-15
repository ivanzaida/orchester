use std::{
  collections::{HashMap, VecDeque},
  hash::Hash,
};

/// A limiter's map with at most `max` keys (review 2, N2). A new key beyond that evicts the oldest key that may go
/// rather than being refused, so a flood of new keys cannot lock everyone out; it forgets only what the flood itself
/// pushed out, and the limits on other keys (the client's address) keep holding. When no key may go (each one is
/// in use), the map grows past its bound rather than refuse.
pub(super) struct Bounded<K, V> {
  map: HashMap<K, V>,
  /// Keys in the order they were added; may hold keys pruned since, which eviction skips.
  order: VecDeque<K>,
  max: usize,
}

impl<K: Hash + Eq + Clone, V: Default> Bounded<K, V> {
  pub fn new(max: usize) -> Self {
    Self {
      map: HashMap::new(),
      order: VecDeque::new(),
      max,
    }
  }

  pub fn get(&self, key: &K) -> Option<&V> {
    self.map.get(key)
  }

  pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
    self.map.get_mut(key)
  }

  /// The key's value, added empty when missing, after evicting the oldest key `evictable` lets go when full.
  pub fn entry(&mut self, key: K, evictable: impl Fn(&V) -> bool) -> &mut V {
    if !self.map.contains_key(&key) {
      self.make_room(evictable);
      self.order.push_back(key.clone());
    }
    self.map.entry(key).or_default()
  }

  fn make_room(&mut self, evictable: impl Fn(&V) -> bool) {
    let mut turns = self.order.len();
    while self.map.len() >= self.max && turns > 0 {
      turns -= 1;
      let Some(oldest) = self.order.pop_front() else {
        return;
      };
      match self.map.get(&oldest) {
        Some(value) if evictable(value) => {
          self.map.remove(&oldest);
        }
        Some(_) => self.order.push_back(oldest),
        None => {}
      }
    }
  }

  pub fn retain(&mut self, keep: impl Fn(&V) -> bool) {
    self.map.retain(|_, value| keep(value));
    let map = &self.map;
    self.order.retain(|key| map.contains_key(key));
  }
}
