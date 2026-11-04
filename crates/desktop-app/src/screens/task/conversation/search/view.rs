//! What the open search makes of the tab's timeline: the marks its rows draw, the rows it opens and reveals for the
//! current place, the find bar over the chat with the search run under it, and the track of marks at the chat's end.
//! A closed search folds again what it opened, and leaves the chat where it is.

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tasks::timeline::{
  find::{Hit, Needle},
  view::Row,
};
use parking_lot::Mutex;

use super::{
  TranscriptSearch,
  bar::{Count, bar},
  marks::{Marks, track},
  row_of,
  run::{Outcome, RunProps, SameItems, SearchRun},
  to_open,
};
use crate::{
  screens::task::conversation::{built::Built, timeline::Toggles},
  ui::chat::follow::Follow,
};

/// The open search, as the tab shows it.
pub(in super::super) struct Searching {
  /// What the rows mark.
  pub(in super::super) marks: Marks,
  /// The row of the current place, which the chat brings into view.
  pub(in super::super) reveal: Option<String>,
  /// The find bar, with the search run that hands it what it found.
  pub(in super::super) bar: Element,
  /// The marks on the chat's track.
  pub(in super::super) track: Element,
}

/// What the tab keeps of its search: what the search run found, and the place last made current.
pub(in super::super) struct SearchHold {
  pub(in super::super) outcome: Signal<Outcome>,
  pub(in super::super) current: Mutex<Current>,
}

/// The place last made current, and the rows the search opened to show it.
#[derive(Default)]
pub(in super::super) struct Current {
  hit: Option<Hit>,
  opened: HashSet<Arc<str>>,
}

/// The search of `built`, the tab's timeline of a transcript of `entries` entries, while it is open; `toggles` open
/// what hides its current place, and `follow` stops following once it moves to a place. Closed, it folds again what it
/// opened.
pub(in super::super) fn searching(
  ctx: &mut Ctx,
  search: &TranscriptSearch,
  (toggles, follow): (&Toggles, &Follow),
  (built, entries): (&Built, usize),
  hold: &SearchHold,
) -> Option<Searching> {
  if !search.is_open() {
    toggles.fold_searched();
    *hold.current.lock() = Current::default();
    return None;
  }
  let needle = Needle::new(&search.query.get());
  let run = ctx.mount_keyed::<SearchRun>(
    "conversation-search-run",
    RunProps {
      items: SameItems(built.items.clone()),
      needle: needle.clone(),
      at: search.at.clone(),
      outcome: hold.outcome.clone(),
    },
  );
  let outcome = hold.outcome.get();
  // Until the run hands over what it found for the query typed now, the bar says it is searching.
  let fresh = outcome.needle == needle;
  let found = if fresh { outcome.found.clone() } else { Arc::default() };
  let searching = needle.is_some() && (!fresh || outcome.searching);
  let count = found.hits.len();
  let current = (count > 0).then(|| search.at.get().min(count - 1));
  let hit = current.and_then(|at| found.hits.get(at));
  let reveal = hit.and_then(|hit| show(hit, &built.rows, (toggles, follow), &hold.current));
  let rows = row_index(&built.rows);
  // A place in a folded step sits where the fold is.
  let row_of_hit = |hit: &Hit| {
    rows
      .get(&*hit.item)
      .or_else(|| hit.step.as_ref().and_then(|step| rows.get(&**step)))
      .copied()
  };
  let go = |index: usize| -> Box<dyn Fn() + Send + Sync> {
    let at = search.at.clone();
    Box::new(move || at.set(index))
  };
  let track = track((&found.hits, current), built.rows.len(), &row_of_hit, &go);
  let count = Count {
    found: count,
    current,
    searching,
    entries,
  };
  let bar = Column::new()
    .padding(Padding::new().top(SpacingSize::Sm).right(SpacingSize::Section))
    .child(bar(ctx, search, count))
    .child(run)
    .into();
  let track = Column::new()
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .child(track)
    .into();
  Some(Searching {
    marks: Marks::new(needle, found, current),
    reveal,
    bar,
    track,
  })
}

/// Opens what hides `hit`, the current place, and the key of the row that shows it. A place made current anew stops
/// the chat from following, so the chat goes to it and stays. Each row is opened once for the place: one the person
/// folds again stays folded until another place is made current.
fn show(
  hit: &Hit,
  rows: &[Arc<Row>],
  (toggles, follow): (&Toggles, &Follow),
  shown: &Mutex<Current>,
) -> Option<String> {
  let mut shown = shown.lock();
  if shown.hit.as_ref() != Some(hit) {
    shown.hit = Some(hit.clone());
    shown.opened.clear();
    follow.hold_in_place();
  }
  // Opening a step can bring up rows that hide the place too, so what is still to open is asked at each render.
  let keys: Vec<Arc<str>> = to_open(hit, rows, toggles)
    .into_iter()
    .filter(|key| shown.opened.insert(key.clone()))
    .collect();
  if !keys.is_empty() {
    toggles.open_for_search(keys);
  }
  row_of(rows, &hit.item).map(|key| key.to_string())
}

/// The index of the row that shows each item, its own or one of its tool groups and Autopilot's requests.
fn row_index(rows: &[Arc<Row>]) -> HashMap<&str, usize> {
  let mut index = HashMap::new();
  for (at, row) in rows.iter().enumerate() {
    match &**row {
      Row::Agent(agent) => {
        index.insert(&*agent.key, at);
        for chip in &agent.chips {
          index.insert(&*chip.key, at);
        }
      }
      Row::Approvals { items, .. } => {
        for item in items {
          index.insert(&*item.key, at);
        }
      }
      Row::Folded { steps, .. } => {
        for (step, _) in steps {
          index.insert(&**step, at);
        }
      }
      other => {
        index.insert(&**other.key(), at);
      }
    }
  }
  index
}
