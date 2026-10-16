//! Running the search within the file off the UI thread, so a keystroke in a large file does not stall the
//! window, and keeping its current place: a new query starts at its first place; the same query in a text read
//! again stays at the place the current one was at ([`TextMatch::same_place`]), so following changes does not
//! move the file. A scan a newer one superseded stops. While the same query runs on a text read again, what it
//! found stays shown; while a new query runs, the search says it is searching.

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_workspace::{
  background,
  files::{FileText, TextMatch},
  messages::Message,
};
use parking_lot::Mutex;

use super::{find::FindState, reads::Same};

/// What a search looks for: its text and whether case counts.
type Query = Option<(String, bool)>;

/// What a search found, for the text and the query it ran with.
#[derive(Clone, PartialEq)]
struct Searched {
  text: Same<FileText>,
  query: Query,
  found: Arc<Vec<TextMatch>>,
}

impl DevtoolsInspectable for Searched {}

/// What the search shows: the places found, and whether a new query is still being searched for.
pub struct Places {
  pub found: Arc<Vec<TextMatch>>,
  pub searching: bool,
}

/// The pane's search: what it shows found, and the flag of the scan under way.
#[derive(Default)]
pub struct Search {
  shown: Mutex<Option<Searched>>,
  scanning: Arc<Mutex<Arc<AtomicBool>>>,
}

impl Search {
  /// The places `find` looks for in `text`, once a background thread found them.
  pub fn found(&self, ctx: &mut Ctx, text: &Arc<FileText>, find: &FindState) -> Places {
    let key = (Same(text.clone()), find.looking_for());
    let scanning = self.scanning.clone();
    let state = ctx
      .future(key.clone(), move |(text, query)| {
        let wanted = Arc::new(AtomicBool::new(true));
        let earlier = std::mem::replace(&mut *scanning.lock(), wanted.clone());
        earlier.store(false, Ordering::Relaxed);
        async move {
          let found = match query.clone() {
            Some((looked_for, match_case)) => {
              let searched = text.0.clone();
              let still = move || wanted.load(Ordering::Relaxed);
              // A superseded scan finds nothing; what it was for is no longer shown.
              background::run(move || searched.find(&looked_for, match_case, &still))
                .await?
                .unwrap_or_default()
            }
            None => Vec::new(),
          };
          Ok::<_, Message>(Searched {
            text,
            query,
            found: Arc::new(found),
          })
        }
      })
      .state()
      .get();
    let fresh = state
      .data
      .filter(|searched| searched.text == key.0 && searched.query == key.1);
    let mut shown = self.shown.lock();
    let Some(searched) = fresh else {
      let same_query = shown.as_ref().filter(|before| before.query == key.1);
      return Places {
        found: same_query.map_or_else(|| Arc::new(Vec::new()), |before| before.found.clone()),
        searching: key.1.is_some() && same_query.is_none(),
      };
    };
    let unchanged = shown
      .as_ref()
      .is_some_and(|before| Arc::ptr_eq(&before.found, &searched.found));
    if !unchanged {
      let at = shown
        .as_ref()
        .map_or(0, |before| next_place(before, &searched, find.at.get_untracked()));
      if at != find.at.get_untracked() {
        find.at.set(at);
      }
      *shown = Some(searched.clone());
    }
    Places {
      found: searched.found,
      searching: false,
    }
  }
}

/// The current place once `searched` replaces `before`, from the place `at` among what `before` found.
fn next_place(before: &Searched, searched: &Searched, at: usize) -> usize {
  if before.query != searched.query {
    return 0;
  }
  let current = before.found.get(at.min(before.found.len().saturating_sub(1)));
  current
    .and_then(|place| TextMatch::same_place(&searched.found, *place))
    .unwrap_or(0)
}
