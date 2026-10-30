//! The run's timeline as the tab shows it, built again only when what it is built from changed (ORC-71): the
//! transcript and requests as read, the task, the conversation and the rows the person opened or folded. The tab
//! renders again for what changes none of them (another read of the page, the chat following its end or not), and
//! building a long run's timeline is the tab's costliest work: 8 ms a render for 1,250 entries in a release build,
//! twice as the chat opened. Its rows are shared with the list, which then compares them by address. What the merge
//! queue did with the run (ORC-19) comes with them, each line with the row it goes before.

use std::sync::Arc;

use orchester_workspace::tasks::{
  conversation::{Conversation, orchestrator::continued},
  detail::{RunView, TaskDetail},
  timeline::{
    self as items, Item, Sources, delivery,
    load::Loaded,
    view::{self, Choice, Row},
  },
};
use parking_lot::Mutex;

use super::timeline::{OpenRows, Placed, without_usage};
use crate::ui::markdown::plain;

/// The timeline's items, the rows the chat shows of them, and the merge queue's lines with the row each goes before.
#[derive(Clone)]
pub(super) struct Built {
  pub(super) items: Arc<Vec<Item>>,
  pub(super) rows: Arc<Vec<Arc<Row>>>,
  pub(super) delivery: Arc<Vec<Placed>>,
}

/// What a timeline was built from.
struct Source {
  loaded: Loaded,
  detail: Arc<TaskDetail>,
  conversation: Conversation,
  open: OpenRows,
}

/// The timeline built last, and what it was built from.
#[derive(Default)]
pub(super) struct BuiltTimeline(Mutex<Option<(Source, Built)>>);

impl BuiltTimeline {
  /// The timeline of `run`, the latest run of `detail`'s task, from `loaded` with `conversation` and the rows the
  /// person `open`ed or folded: the one built last when it was built from the same.
  pub(super) fn of(
    &self,
    (run, detail): (&RunView, &Arc<TaskDetail>),
    (loaded, conversation): (&Loaded, &Conversation),
    open: OpenRows,
  ) -> Built {
    let mut last = self.0.lock();
    if let Some((source, built)) = &*last
      && source.loaded == *loaded
      && source.detail == *detail
      && source.conversation == *conversation
      && source.open == open
    {
      return built.clone();
    }
    let mut items = items::build(&Sources {
      run,
      entries: &loaded.entries,
      conversation,
      pending: &loaded.pending,
      reviewing: &loaded.reviewing,
      decided: &loaded.decided,
      answers: &loaded.answers,
      questions: &loaded.questions,
      merge: detail.merge.as_ref(),
    });
    continued(&mut items, &loaded.orchestrated);
    let shown = without_usage(view::shown(
      &items,
      &Choice::default(),
      &|key: &str, default: bool| open.is_open(key, default),
      &plain::rendered,
    ));
    let rows: Vec<Arc<Row>> = shown.rows.into_iter().map(Arc::new).collect();
    let lines = detail
      .queue
      .as_ref()
      .and_then(|queue| queue.delivery.as_ref())
      .map(delivery::lines)
      .unwrap_or_default();
    let placed = delivery::places(&rows, &lines)
      .into_iter()
      .zip(lines.into_iter().map(Arc::new))
      .collect();
    let built = Built {
      items: Arc::new(items),
      rows: Arc::new(rows),
      delivery: Arc::new(placed),
    };
    *last = Some((
      Source {
        loaded: loaded.clone(),
        detail: detail.clone(),
        conversation: conversation.clone(),
        open,
      },
      built.clone(),
    ));
    built
  }
}
