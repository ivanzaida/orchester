//! What the review page keeps while it shows: the comment box's lines, kind and words, the line a drag over lines
//! started on, the thread whose follow-up or edit is open, the files folded or unfolded by hand, the folds opened, the
//! file the list scrolls to, the head the person took in after it moved, and why the latest change was not made.

use std::{
  collections::{BTreeMap, BTreeSet},
  sync::Arc,
};

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};
use orchester_core::{
  id::{ReviewMessageId, ReviewThreadId},
  pull_request::feedback::{DiffSide, LineSpan},
  review::ThreadKind,
  revision::Revision,
};
use orchester_workspace::messages::Message;
use parking_lot::Mutex;

use crate::screens::task::changes::diff::LineKey;

/// A value of the domain the page keeps in a signal: the domain's types know nothing of DevTools, which shows this
/// one without looking into it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(super) struct Held<T>(pub T);

impl<T> DevtoolsInspectable for Held<T> {}

/// The line a drag over lines started on, in the file at its path.
type DragStart = Option<(Arc<str>, LineKey)>;

/// The lines the comment box is open under, chosen in the diff of `head`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Composing {
  pub path: Arc<str>,
  pub side: DiffSide,
  pub span: LineSpan,
  pub head: Revision,
}

impl Composing {
  /// The line the box stands under.
  pub fn end(&self) -> LineKey {
    LineKey {
      side: self.side,
      number: self.span.end(),
    }
  }

  /// Whether `line` of the file at `path` is among the chosen lines.
  pub fn holds(&self, path: &str, line: LineKey) -> bool {
    *self.path == *path && line.side == self.side && self.span.contains(line.number)
  }
}

/// What a box inside a thread writes: *Ask again* or *Reply* after it, or the words of a comment not sent yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum InThread {
  FollowUp(ReviewThreadId),
  Edit(ReviewMessageId),
}

/// The page's own state, shared by its parts.
#[derive(Clone)]
pub(super) struct ReviewState {
  pub composing: Signal<Held<Option<Composing>>>,
  pub kind: Signal<Held<ThreadKind>>,
  pub text: Signal<String>,
  /// The line a press started a drag on, in the file at its path; `None` once the button is up.
  pub drag: Arc<Mutex<DragStart>>,
  pub in_thread: Signal<Held<Option<InThread>>>,
  pub in_thread_text: Signal<String>,
  /// Files folded or unfolded by hand, by path; the others fold once they are marked *Viewed*.
  pub folded: Signal<Held<BTreeMap<Arc<str>, bool>>>,
  /// The folds of outdated and resolved threads opened, by their keys.
  pub opened: Signal<Held<BTreeSet<Arc<str>>>>,
  /// The file chosen in the list, which the diff scrolls to.
  pub chosen: Signal<Held<Option<Arc<str>>>>,
  /// The head the person took in after the branch moved past the one reviewed.
  pub taken_in: Signal<Held<Option<Revision>>>,
  /// Why the latest change was not made.
  pub failure: Signal<Option<Message>>,
  /// Counts the changes made here: the review is read again after each.
  pub written: Signal<u64>,
  /// *Discard* asks whether every comment not sent goes.
  pub discarding: Signal<bool>,
}

impl PartialEq for ReviewState {
  fn eq(&self, other: &Self) -> bool {
    self.text.id() == other.text.id()
  }
}

impl DevtoolsInspectable for ReviewState {}

impl ReviewState {
  /// The state of a page whose changes count in `written`, which the page's read follows.
  pub fn new(ctx: &mut Ctx, written: Signal<u64>) -> Self {
    Self {
      composing: ctx.signal(Held(None)),
      kind: ctx.signal(Held(ThreadKind::Comment)),
      text: ctx.signal(String::new()),
      drag: Arc::default(),
      in_thread: ctx.signal(Held(None)),
      in_thread_text: ctx.signal(String::new()),
      folded: ctx.signal(Held(BTreeMap::new())),
      opened: ctx.signal(Held(BTreeSet::new())),
      chosen: ctx.signal(Held(None)),
      taken_in: ctx.signal(Held(None)),
      failure: ctx.signal(None),
      written,
      discarding: ctx.signal(false),
    }
  }

  /// Opens the comment box under `composing`, a comment unless the person chose otherwise for the words kept.
  pub fn compose(&self, composing: Composing) {
    if self.composing.get_untracked().0.is_none() {
      self.kind.set(Held(ThreadKind::Comment));
    }
    self.composing.set(Held(Some(composing)));
  }

  /// Closes the comment box; `sent`: its words went, so they are not kept.
  pub fn close(&self, sent: bool) {
    self.composing.set(Held(None));
    if sent {
      self.text.set(String::new());
    }
  }

  /// Opens a box inside a thread with `text`.
  pub fn open_in_thread(&self, what: InThread, text: String) {
    self.in_thread_text.set(text);
    self.in_thread.set(Held(Some(what)));
  }

  pub fn close_in_thread(&self) {
    self.in_thread.set(Held(None));
    self.in_thread_text.set(String::new());
  }

  /// Starts a drag over lines on `line` of the file at `path`: the box opens under that line alone.
  pub fn press(&self, (path, line): (Arc<str>, LineKey), head: &Revision) {
    let Some(number) = std::num::NonZeroU32::new(line.number) else {
      return;
    };
    *self.drag.lock() = Some((path.clone(), line));
    self.compose(Composing {
      path,
      side: line.side,
      span: LineSpan::line(number),
      head: head.clone(),
    });
  }

  /// The pointer entered `line` of the file at `path` while a drag goes on there: the box takes the lines from the
  /// drag's first to it, on the same side.
  pub fn drag_over(&self, (path, line): (&str, LineKey), head: &Revision) {
    let Some((from_path, from)) = self.drag.lock().clone() else {
      return;
    };
    if *from_path != *path || from.side != line.side {
      return;
    }
    let (start, end) = (from.number.min(line.number), from.number.max(line.number));
    let span = std::num::NonZeroU32::new(start)
      .zip(std::num::NonZeroU32::new(end))
      .and_then(|(start, end)| LineSpan::new(start, end).ok());
    if let Some(span) = span {
      self.compose(Composing {
        path: from_path,
        side: line.side,
        span,
        head: head.clone(),
      });
    }
  }

  /// The button is up: the drag ends where it is.
  pub fn release(&self) {
    *self.drag.lock() = None;
  }

  /// Whether the file at `path` shows folded: as the person folded it, else once it is marked *Viewed*.
  pub fn is_folded(&self, path: &str, viewed: bool) -> bool {
    self.folded.get().0.get(path).copied().unwrap_or(viewed)
  }

  pub fn fold(&self, path: Arc<str>, folded: bool) {
    self.folded.update(|held| {
      held.0.insert(path, folded);
    });
  }

  /// Opens or closes the fold of `key`.
  pub fn toggle_fold(&self, key: Arc<str>) {
    self.opened.update(|held| {
      if !held.0.remove(&key) {
        held.0.insert(key);
      }
    });
  }
}
