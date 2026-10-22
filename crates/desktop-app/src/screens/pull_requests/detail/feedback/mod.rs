//! A pull request's agent feedback on its page (`Pull request feedback` `seN1p`): the Agent feedback tab
//! ([`tab`]), the THREADS list beside Files changed ([`side`]) and the threads and composer drawn into the diff
//! ([`diff`]). The page keeps what the person chose and wrote ([`FeedbackState`]) for as long as it shows, so a
//! composer closed with Esc keeps its draft; the threads are read from the project ([`read_board`]) whenever the
//! project's runs change or a write landed, and every write goes through one action ([`Writes`]) that sends what
//! Orchester may send at once and tells the runner. Where a thread stands, and why, is the workspace's.

mod composer;
mod diff;
mod list;
mod moved;
mod panel;
mod parts;
mod reply;
mod side;
mod tab;
mod thread;
mod words;

use std::{
  collections::HashMap,
  num::NonZeroU64,
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
};
use orchester_core::{
  diff::Diff,
  id::{FeedbackId, FeedbackPostId, IdempotencyKey, MessageId, ResourceId},
  pull_request::feedback::{DiffSide, FeedbackKind, LineSpan},
  revision::Revision,
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::Message,
  pull_requests::feedback::{
    FeedbackBoard, FeedbackError, Inputs, NewThread, Slots, dispatch_in, load_board_in, reply_in, resolve_in, retry_in,
    start_in, tell_runner, withdraw_in,
  },
};
use parking_lot::Mutex;

pub(super) use self::{
  diff::annotations,
  side::threads,
  tab::{TabProps, feedback_tab, read_failed},
  thread::Env,
};
use crate::{
  project::{ProjectFeed, ShownProject, feedback_holds},
  services::Services,
};

/// Which threads the Agent feedback tab lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
  #[default]
  Open,
  Resolved,
}

impl DevtoolsInspectable for Filter {}

/// Which kind of request the list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KindFilter {
  #[default]
  All,
  Questions,
  Fixes,
}

impl DevtoolsInspectable for KindFilter {}

impl KindFilter {
  pub const fn shows(self, kind: FeedbackKind) -> bool {
    matches!(
      (self, kind),
      (Self::All, _) | (Self::Questions, FeedbackKind::Question) | (Self::Fixes, FeedbackKind::Fix)
    )
  }
}

/// A composer the page has open, or had and keeps the draft of.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Composing {
  /// A new thread on lines of a file, chosen at the head `revision`.
  Lines {
    path: Arc<str>,
    side: DiffSide,
    span: LineSpan,
    revision: Revision,
  },
  /// A new thread on the whole pull request.
  PullRequest,
  /// A reply in a thread.
  Reply(FeedbackId),
}

impl DevtoolsInspectable for Composing {}

/// A value of the domain the page keeps in a signal, a select or a read's dependencies: the domain's types know
/// nothing of DevTools, which shows this one without looking into it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Held<T>(pub T);

impl<T> DevtoolsInspectable for Held<T> {}

/// What a person wrote in one composer: kept until it is sent, also while the composer is closed.
#[derive(Clone)]
pub struct Draft {
  pub text: Signal<String>,
  pub kind: Signal<Held<FeedbackKind>>,
  /// The agent it goes to; `None` until the person picks one: the board's default then.
  pub target: Signal<Held<Option<ResourceId>>>,
  /// When it was last changed; `None` while nothing was written.
  pub saved_at: Signal<Held<Option<Timestamp>>>,
  /// Sends it once whatever retries it, until it is sent.
  pub key: IdempotencyKey,
}

/// The drafts of the page's composers.
#[derive(Clone, Default)]
pub struct Drafts(Arc<Mutex<HashMap<Composing, Draft>>>);

impl Drafts {
  /// The draft of `composing`, made when there is none: a reply asks what the thread asks, a new thread a question.
  pub fn of(&self, composing: &Composing, kind: FeedbackKind) -> Draft {
    self
      .0
      .lock()
      .entry(composing.clone())
      .or_insert_with(|| Draft {
        text: Signal::new(String::new()),
        kind: Signal::new(Held(kind)),
        target: Signal::new(Held(None)),
        saved_at: Signal::new(Held(None)),
        key: IdempotencyKey::generate(),
      })
      .clone()
  }

  /// Moves the draft of `from` to `to`, e.g. a new thread's lines chosen again at a newer head.
  pub fn moved(&self, from: &Composing, to: Composing) {
    let mut drafts = self.0.lock();
    if let Some(draft) = drafts.remove(from) {
      drafts.insert(to, draft);
    }
  }

  /// Forgets the draft of `composing` once it was sent.
  pub fn sent(&self, composing: &Composing) {
    self.0.lock().remove(composing);
  }

  /// How many new threads are drafted but not sent.
  pub fn new_threads(&self) -> usize {
    self
      .0
      .lock()
      .iter()
      .filter(|(composing, draft)| {
        !matches!(composing, Composing::Reply(_)) && !draft.text.get_untracked().trim().is_empty()
      })
      .count()
  }
}

impl PartialEq for Drafts {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

/// What the page keeps of its feedback while it shows.
#[derive(Clone)]
pub struct FeedbackState {
  pub filter: Signal<Filter>,
  pub kind: Signal<KindFilter>,
  /// The composer that is open.
  pub composing: Signal<Option<Composing>>,
  pub drafts: Drafts,
  /// The thread Files changed scrolls to.
  pub reveal: Signal<Held<Option<FeedbackId>>>,
  /// Counts the writes that landed: the threads are read again after each.
  pub written: Signal<u64>,
  /// Why the latest write did not land.
  pub failure: Signal<Option<Message>>,
  /// The head Files changed shows instead of the pull request's latest, one a thread was written at (*Show at*).
  pub pinned: Signal<Held<Option<Revision>>>,
  /// Files changed shows the pull request's head although a new thread is being written at an older one: the person
  /// asked to see it (*Show …*, *Show what changed*). Otherwise the head the draft was started at stays shown.
  pub follow: Signal<bool>,
  /// Files changed shows what changed since this thread's request (*Since your request*).
  pub since: Signal<Held<Option<FeedbackId>>>,
}

impl PartialEq for FeedbackState {
  fn eq(&self, other: &Self) -> bool {
    self.filter.id() == other.filter.id()
  }
}

impl DevtoolsInspectable for FeedbackState {}

impl FeedbackState {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      filter: ctx.signal(Filter::Open),
      kind: ctx.signal(KindFilter::All),
      composing: ctx.signal(None),
      drafts: Drafts::default(),
      reveal: ctx.signal(Held(None)),
      written: ctx.signal(0),
      failure: ctx.signal(None),
      pinned: ctx.signal(Held(None)),
      follow: ctx.signal(false),
      since: ctx.signal(Held(None)),
    }
  }

  /// The head Files changed shows: one pinned, else the one a new thread is being written at when the pull request
  /// moved on and the person did not ask to follow it; `None`: the pull request's head.
  pub fn shown_head(&self, head: Option<&Revision>) -> Option<Revision> {
    if let Held(Some(pinned)) = self.pinned.get() {
      return Some(pinned);
    }
    match self.composing.get() {
      Some(Composing::Lines { revision, .. }) if Some(&revision) != head && !self.follow.get() => Some(revision),
      _ => None,
    }
  }
}

/// A write to the pull request's feedback.
#[derive(Clone, Debug)]
pub enum Write {
  Start(IdempotencyKey, NewThread),
  Reply {
    key: IdempotencyKey,
    thread: FeedbackId,
    kind: FeedbackKind,
    text: String,
    revision: Revision,
  },
  Withdraw(FeedbackId, FeedbackPostId),
  Resolve(FeedbackId, bool),
  Retry(FeedbackId, MessageId),
}

/// The composer a write sends from, which closes once it landed.
type Sent = Option<Composing>;

/// What the hold rules know of the project's runner as a write is asked for ([`feedback_holds`]).
type Holds = (Option<Slots>, bool);

/// Every write to the pull request's feedback, one at a time: the write, then what Orchester may send at once under
/// the hold rules the project's follower applies, then the runner told of it. A write asked for while another runs is
/// refused, so the one in flight is never replaced and its sending never dropped.
#[derive(Clone)]
pub struct Writes {
  action: FutureAction<(Write, Sent, Holds), Sent, Message>,
  feed: ProjectFeed,
  root: PathBuf,
}

impl PartialEq for Writes {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

impl Writes {
  pub fn new(ctx: &mut Ctx, (project, number): (&ShownProject, NonZeroU64), state: &FeedbackState) -> Self {
    let services = Services::of(ctx);
    let open = project.open.clone();
    let action = ctx.future_action(move |(write, sent, (slots, lost)): (Write, Sent, Holds)| {
      let (services, open) = (services.clone(), open.clone());
      async move {
        let root = open.repo_root.clone();
        background::run(move || apply(&root, number.get(), write))
          .await?
          .map_err(|error| {
            tracing::warn!(%error, "a feedback write did not land");
            error.message()
          })?;
        let root = open.repo_root.clone();
        match background::run(move || dispatch_in(&root, slots, lost)).await? {
          Ok(dispatched) => tell_runner(&services.runners, &services.servers, &open, &dispatched).await,
          Err(error) => tracing::warn!(%error, "held feedback was not sent after a write"),
        }
        Ok::<Sent, Message>(sent)
      }
    });
    let (written, failure, composing, drafts) = (
      state.written.clone(),
      state.failure.clone(),
      state.composing.clone(),
      state.drafts.clone(),
    );
    ctx.watch(&action.state(), move |landed| {
      if landed.is_pending() {
        return;
      }
      if let Some(error) = &landed.error {
        failure.set(Some(error.clone()));
        return;
      }
      failure.set(None);
      written.update(|count| *count += 1);
      if let Some(Some(sent)) = &landed.data {
        drafts.sent(sent);
        if composing.get_untracked().as_ref() == Some(sent) {
          composing.set(None);
        }
      }
    });
    Self {
      action,
      feed: project.feed.clone(),
      root: project.open.repo_root.clone(),
    }
  }

  /// Runs `write`, unless another write runs; `sent`: the composer it was written in.
  pub fn run(&self, write: Write, sent: Sent) {
    if self.action.state().get_untracked().is_pending() {
      return;
    }
    let holds = feedback_holds(self.feed.status.get_untracked().as_ref(), &self.root);
    self.action.run((write, sent, holds));
  }

  pub fn busy(&self) -> bool {
    self.action.state().get().is_pending()
  }
}

fn apply(root: &Path, number: u64, write: Write) -> Result<(), FeedbackError> {
  match write {
    Write::Start(key, new) => start_in(root, key, &new).map(drop),
    Write::Reply {
      key,
      thread,
      kind,
      text,
      revision,
    } => reply_in(root, key, (number, thread), (kind, &text, &revision)).map(drop),
    Write::Withdraw(thread, post) => withdraw_in(root, (number, thread), post),
    Write::Resolve(thread, resolved) => resolve_in(root, (number, thread), resolved).map(drop),
    Write::Retry(thread, message) => retry_in(root, (number, thread), message),
  }
}

/// The threads as read, and why the latest read failed: then `board` is the one read before, which the page shows
/// under that failure.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardRead {
  pub board: Option<Arc<FeedbackBoard>>,
  pub failed: Option<Message>,
}

#[derive(Clone, Debug, PartialEq)]
struct Read(Arc<FeedbackBoard>);

impl DevtoolsInspectable for Read {}

/// The pull request's feedback, read off the UI thread when the page opens, whenever the project's runs change, a
/// write landed or the pull request's head or diff is read again (`head`). What was read before shows meanwhile.
pub fn read_board(
  ctx: &mut Ctx,
  (project, number): (&ShownProject, NonZeroU64),
  head: Option<(Revision, Option<Arc<Diff>>)>,
  written: u64,
) -> BoardRead {
  let programs = Services::of(ctx).programs;
  let root = project.open.repo_root.clone();
  let (slots, lost) = feedback_holds(project.feed.status.get().as_ref(), &root);
  let live = project.feed.running.get();
  let changes = project.feed.run_changes(live);
  let diff_key = head
    .as_ref()
    .and_then(|(_, diff)| diff.as_ref().map(|diff| Arc::as_ptr(diff) as usize));
  let head_key = head.as_ref().map(|(revision, _)| revision.clone());
  let deps = (root, number, head_key, diff_key, changes, written, lost, slots);
  let load = ctx.future(deps, move |(root, number, ..)| {
    let (programs, head) = (programs.clone(), head.clone());
    async move {
      background::run(move || {
        let inputs = Inputs {
          root: &root,
          programs: &programs,
          head: head.as_ref().map(|(revision, diff)| (revision, diff.as_deref())),
          slots,
          lost,
        };
        load_board_in(&inputs, number.get())
      })
      .await?
      .map(|board| Read(Arc::new(board)))
      .map_err(|error| {
        tracing::warn!(%error, "cannot read the pull request's feedback");
        error.message()
      })
    }
  });
  let state = load.state().get();
  BoardRead {
    board: state.data.map(|Read(board)| board),
    failed: state.error,
  }
}
