//! Running the search off the UI thread, so a keystroke in a long run's transcript does not stall the window, and
//! keeping its current place. A component of its own ([`SearchRun`]) asks for the search and hands what it found to the
//! tab ([`Outcome`]), so its future never moves the tab's own. A new query waits 150 ms for typing to stop, then
//! searches; a scan a newer one superseded stops. While the same query runs again on the run's newer timeline, what it
//! found stays shown and the current place stays the place it was ([`Hit::same_place`]): new places add to the count
//! without moving it. While a new query runs, the search says it is searching.

use std::{
  collections::HashMap,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_workspace::{
  background,
  messages::Message,
  tasks::timeline::{
    Item,
    find::{Hit, Needle, Spot, find},
  },
};
use parking_lot::Mutex;

use crate::ui::markdown::plain;

/// How long typing must stop before a new query is searched for (13f `aoVlE`).
const TYPING_PAUSE: Duration = Duration::from_millis(150);

/// The timeline's items, compared by which build they are: a build makes new ones only when the run changed.
#[derive(Clone)]
pub(in super::super) struct SameItems(pub(in super::super) Arc<Vec<Item>>);

impl PartialEq for SameItems {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for SameItems {}

/// The places found, in the chat's order, and where they are: by item, and how many each step's fold hides.
#[derive(Debug, Default)]
pub(in super::super) struct Found {
  pub(in super::super) hits: Vec<Hit>,
  by_item: HashMap<Arc<str>, Vec<usize>>,
  by_step: HashMap<Arc<str>, usize>,
}

impl Found {
  fn of(hits: Vec<Hit>) -> Self {
    let mut by_item: HashMap<Arc<str>, Vec<usize>> = HashMap::new();
    let mut by_step: HashMap<Arc<str>, usize> = HashMap::new();
    for (index, hit) in hits.iter().enumerate() {
      by_item.entry(hit.item.clone()).or_default().push(index);
      if let Some(step) = &hit.step {
        *by_step.entry(step.clone()).or_default() += 1;
      }
    }
    Self { hits, by_item, by_step }
  }

  /// The indices of the places in the item `item`.
  pub(in super::super) fn in_item(&self, item: &str) -> &[usize] {
    self.by_item.get(item).map_or(&[], Vec::as_slice)
  }

  /// How many places the step `step`'s fold hides.
  pub(in super::super) fn in_step(&self, step: &str) -> usize {
    self.by_step.get(step).copied().unwrap_or_default()
  }

  /// Whether the item `item` has a place in what it says (not in a call of a tool group).
  pub(in super::super) fn said_in(&self, item: &str) -> bool {
    self
      .in_item(item)
      .iter()
      .any(|&index| self.hits[index].spot == Spot::Said)
  }
}

/// What the search shows: the places found for the needle, and whether a new query is still being searched for.
#[derive(Clone, Debug, Default)]
pub(in super::super) struct Outcome {
  pub(in super::super) needle: Option<Needle>,
  pub(in super::super) found: Arc<Found>,
  pub(in super::super) searching: bool,
}

impl PartialEq for Outcome {
  fn eq(&self, other: &Self) -> bool {
    self.needle == other.needle && Arc::ptr_eq(&self.found, &other.found) && self.searching == other.searching
  }
}

impl DevtoolsInspectable for Outcome {}

/// What a search found, for the items and the needle it ran with.
#[derive(Clone)]
struct Searched {
  items: SameItems,
  needle: Option<Needle>,
  found: Arc<Found>,
}

impl PartialEq for Searched {
  fn eq(&self, other: &Self) -> bool {
    self.items == other.items && self.needle == other.needle && Arc::ptr_eq(&self.found, &other.found)
  }
}

impl DevtoolsInspectable for Searched {}

/// The search of `items` for `needle`; what it finds goes to `outcome`, and `at`, the search's current place, stays on
/// the place it was as the run's timeline grows.
#[derive(Clone)]
pub(in super::super) struct RunProps {
  pub(in super::super) items: SameItems,
  pub(in super::super) needle: Option<Needle>,
  pub(in super::super) at: Signal<usize>,
  pub(in super::super) outcome: Signal<Outcome>,
}

/// Compared by what it searches; the signals are the tab's, the same for its life.
impl PartialEq for RunProps {
  fn eq(&self, other: &Self) -> bool {
    self.items == other.items
      && self.needle == other.needle
      && self.at.id() == other.at.id()
      && self.outcome.id() == other.outcome.id()
  }
}

impl DevtoolsInspectable for RunProps {}

/// Asks for the search off the UI thread and hands what it found to the tab; it shows nothing itself.
pub(in super::super) struct SearchRun {
  shown: Mutex<Option<Searched>>,
  scanning: Arc<Mutex<Arc<AtomicBool>>>,
}

impl Component for SearchRun {
  type Props = RunProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      shown: Mutex::new(None),
      scanning: Arc::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunProps>().clone();
    let outcome = self.outcome(ctx, &props);
    if props.outcome.get_untracked() != outcome {
      props.outcome.set(outcome);
    }
    Column::new()
  }
}

impl SearchRun {
  fn outcome(&self, ctx: &mut Ctx, props: &RunProps) -> Outcome {
    let key = (props.items.clone(), props.needle.clone());
    let scanning = self.scanning.clone();
    // A new query waits for typing to stop; the same one on a newer timeline runs at once.
    let typed = self.shown.lock().as_ref().is_none_or(|shown| shown.needle != key.1);
    let state = ctx
      .future(key.clone(), move |(items, needle)| {
        let wanted = Arc::new(AtomicBool::new(true));
        let earlier = std::mem::replace(&mut *scanning.lock(), wanted.clone());
        earlier.store(false, Ordering::Relaxed);
        async move {
          let Some(looked_for) = needle.clone() else {
            return Ok::<_, Message>(Searched {
              items,
              needle,
              found: Arc::default(),
            });
          };
          if typed {
            tokio::time::sleep(TYPING_PAUSE).await;
          }
          let searched = items.0.clone();
          let still = move || wanted.load(Ordering::Relaxed);
          // A superseded scan finds nothing; what it was for is no longer shown.
          let hits = background::run(move || find(&searched, &looked_for, &plain::rendered, &still))
            .await?
            .unwrap_or_default();
          Ok(Searched {
            items,
            needle,
            found: Arc::new(Found::of(hits)),
          })
        }
      })
      .state()
      .get();
    let fresh = state
      .data
      .filter(|searched| searched.items == key.0 && searched.needle == key.1);
    let mut shown = self.shown.lock();
    let Some(searched) = fresh else {
      let same_query = shown.as_ref().filter(|before| before.needle == key.1);
      return Outcome {
        found: same_query.map_or_else(Arc::default, |before| before.found.clone()),
        searching: key.1.is_some() && same_query.is_none(),
        needle: key.1,
      };
    };
    let unchanged = shown
      .as_ref()
      .is_some_and(|before| Arc::ptr_eq(&before.found, &searched.found));
    if !unchanged {
      let now = shown
        .as_ref()
        .map_or(0, |before| next_place(before, &searched, props.at.get_untracked()));
      if now != props.at.get_untracked() {
        props.at.set(now);
      }
      *shown = Some(searched.clone());
    }
    Outcome {
      needle: searched.needle,
      found: searched.found,
      searching: false,
    }
  }
}

/// The current place once `searched` replaces `before`, from the place `at` among what `before` found: the same place
/// when the query is the same and the place is still there, else the first.
fn next_place(before: &Searched, searched: &Searched, at: usize) -> usize {
  if before.needle != searched.needle {
    return 0;
  }
  let Some(current) = before.found.hits.get(at.min(before.found.hits.len().saturating_sub(1))) else {
    return 0;
  };
  searched
    .found
    .hits
    .iter()
    .position(|hit| hit.same_place(current))
    .unwrap_or(0)
}
