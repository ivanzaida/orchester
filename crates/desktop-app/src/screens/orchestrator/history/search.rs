//! History's search in titles and messages (ORC-307; `13 · History · search · match in messages only` `SZ6QY`, `13b`
//! `cH5KH`, `13c` `rxx2m`, parts `13f` `H0SXf` in `Orchestrator · redesign`; `docs/design/history-search.md`). Titles
//! filter at once; the messages are searched off the UI thread (`orchester_workspace::orchestrator::search`), newest
//! conversation first, a batch at a time, so each result joins its group as it is found. A new query waits 150 ms for
//! typing to stop; typing again starts the search over. Under the field a line says what was found (11 `text_muted`,
//! 2 in at its sides, 6 under the field): *Found in {n} conversations*, while searching a turning loader and how many
//! were searched, with nothing found how many were. A result whose messages hold the words shows one line of them under
//! its title (`Matching line` `srL91`: who said it in `text_muted`, the words in 11.5/1.4 `text_secondary`); each place
//! in it and in the title is filled `warning_muted` with `radius_sm` (lurq's text fill; the words keep their color).

use std::{collections::HashMap, ops::Range, path::Path, sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
    theme::{PaletteColor, RadiusSize},
  },
  components::{Column, Row, Text, TextHighlight, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ChatId, time::Timestamp};
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::search::{InMessages, Speaker, search_in},
  tasks::timeline::find::Needle,
};

use crate::{
  theme::{ExtraSpacing, IconSize, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    markdown::plain,
    message::message_text,
  },
};

/// How long typing must stop before a new query is searched for (`docs/design/history-search.md`).
const TYPING_PAUSE: Duration = Duration::from_millis(150);
/// How many conversations' messages are read between two looks at what was found.
const BATCH: usize = 24;

/// The conversations searched, newest first, each with when it last changed: one that changed is searched again.
pub(super) type Searched = Arc<[(ChatId, Timestamp)]>;

/// What the search of the messages found so far: for which query and conversations, how many of them it searched,
/// what their messages hold, and why it stopped, when it could not read them.
#[derive(Clone, Debug, Default)]
pub(super) struct Outcome {
  needle: Option<Needle>,
  chats: Searched,
  done: usize,
  found: Arc<HashMap<ChatId, InMessages>>,
  failed: Option<Message>,
}

impl PartialEq for Outcome {
  fn eq(&self, other: &Self) -> bool {
    (self.needle == other.needle && self.chats == other.chats && self.done == other.done)
      && Arc::ptr_eq(&self.found, &other.found)
      && self.failed == other.failed
  }
}

impl DevtoolsInspectable for Outcome {}

/// What the list shows of the search for `needle` in `chats`, from what the run found (`outcome`).
#[derive(Clone, Debug)]
pub(super) struct Search {
  pub(super) needle: Needle,
  found: Arc<HashMap<ChatId, InMessages>>,
  /// How many of `total` conversations were searched, while it still searches.
  searching: Option<usize>,
  total: usize,
  failed: Option<Message>,
}

impl Search {
  /// The search for `needle` in `chats` as `outcome` stands: what an earlier run for the same query found stays
  /// while the conversations are searched again.
  pub(super) fn of(needle: Needle, chats: &Searched, outcome: &Outcome) -> Self {
    let same = outcome.needle.as_ref() == Some(&needle);
    let fresh = same && outcome.chats == *chats;
    let done = outcome.done.min(chats.len());
    let searching =
      (!fresh || (done < chats.len() && outcome.failed.is_none())).then_some(if fresh { done } else { 0 });
    Self {
      found: if same { outcome.found.clone() } else { Arc::default() },
      searching,
      total: chats.len(),
      failed: if fresh { outcome.failed.clone() } else { None },
      needle,
    }
  }

  /// What the search found for the conversation of `chat`, titled `title`: the places in its title and what its
  /// messages hold; `None` when neither holds the words.
  pub(super) fn matched(&self, chat: ChatId, title: &str) -> Option<Matched> {
    let title = self.needle.places(title);
    let messages = self.found.get(&chat).cloned().map(Box::new);
    (!title.is_empty() || messages.is_some()).then_some(Matched { title, messages })
  }

  /// The line under the field: what was found among the conversations, or how far the search got.
  pub(super) fn status(&self, ctx: &Ctx, results: usize) -> Element {
    let (words, loader) = match (&self.failed, self.searching) {
      (Some(reason), _) => {
        let reason = message_text(ctx, reason);
        (
          ctx.t_args("orchestrator.historyPanel.found.failed", [("reason", &*reason)]),
          false,
        )
      }
      (None, Some(done)) => {
        let (done, total) = (done.to_string(), self.total.to_string());
        let words = ctx.t_args(
          "orchestrator.historyPanel.found.searching",
          [("done", done.as_str()), ("total", total.as_str())],
        );
        (words, true)
      }
      (None, None) if results == 0 => {
        let keys = (
          "orchestrator.historyPanel.found.searched.one",
          "orchestrator.historyPanel.found.searched.other",
        );
        (counted(ctx, keys, self.total), false)
      }
      (None, None) => {
        let keys = (
          "orchestrator.historyPanel.found.results.one",
          "orchestrator.historyPanel.found.results.other",
        );
        (counted(ctx, keys, results), false)
      }
    };
    Row::new()
      .id("orchestrator-history-status")
      .describe("searching", self.searching.is_some().to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .with_children(loader.then(|| icon(Icon::LoaderCircle, IconSize::Xs, PaletteColor::TextMuted)))
      .child(
        Text::new(&words)
          .variant(OrchestratorText::HistoryMeta.role())
          .width(Dimension::Pct(100.0))
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      )
      .into()
  }
}

/// The words of `one` for a `count` of 1, else of `other`.
fn counted(ctx: &Ctx, (one, other): (&str, &str), count: usize) -> Arc<str> {
  if count == 1 {
    ctx.t(one)
  } else {
    ctx.t_args(other, [("count", count.to_string().as_str())])
  }
}

/// What the search found in one conversation: the places in its title, and what its messages hold.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Matched {
  pub(super) title: Vec<Range<usize>>,
  pub(super) messages: Option<Box<InMessages>>,
}

impl Matched {
  /// The entry the conversation opens at: the one its first matching line is in.
  pub(super) fn opens_at(&self) -> Option<i64> {
    self.messages.as_ref().map(|messages| messages.line.seq)
  }

  /// The row's meta: `when` and how many matches, where they are.
  pub(super) fn meta(&self, ctx: &Ctx, when: &str) -> Arc<str> {
    let with = |key: &str, count: usize| {
      let count = count.to_string();
      ctx.t_args(key, [("when", when), ("count", count.as_str())])
    };
    match (&self.messages, self.title.is_empty()) {
      (None, _) => ctx.t_args("orchestrator.historyPanel.found.titleOnly", [("when", when)]),
      (Some(messages), true) if messages.places == 1 => with("orchestrator.historyPanel.found.matches.one", 1),
      (Some(messages), true) => with("orchestrator.historyPanel.found.matches.other", messages.places),
      (Some(messages), false) if messages.messages == 1 => {
        with("orchestrator.historyPanel.found.titleAndMessages.one", 1)
      }
      (Some(messages), false) => with(
        "orchestrator.historyPanel.found.titleAndMessages.other",
        messages.messages,
      ),
    }
  }

  /// The line of the messages under the title: who said it, and the words around the first place, marked.
  pub(super) fn line(&self, ctx: &Ctx, chat: ChatId) -> Option<Element> {
    let line = &self.messages.as_ref()?.line;
    let who = match line.who {
      Speaker::Person => "orchestrator.historyPanel.found.you",
      Speaker::Orchestrator => "orchestrator.historyPanel.found.orchestrator",
      Speaker::Report => "orchestrator.historyPanel.found.report",
    };
    let who = ctx.t(who);
    Some(
      Row::new()
        .id(format!("orchestrator-history-line-{chat}"))
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Text::new(&who)
            .variant(OrchestratorText::HistoryLine.role())
            .color(PaletteColor::TextMuted)
            .nowrap(),
        )
        .child(
          Text::new(&line.text)
            .id(format!("orchestrator-history-said-{chat}"))
            .variant(OrchestratorText::HistoryLine.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .nowrap()
            .text_overflow(TextOverflow::Elipsis)
            .highlights(marks(ctx, &line.marks)),
        )
        .into(),
    )
  }
}

/// The fills behind `places` of a row's words.
pub(super) fn marks(ctx: &Ctx, places: &[Range<usize>]) -> Vec<TextHighlight> {
  let radius = ctx.theme().radius_value(RadiusSize::Sm);
  places
    .iter()
    .map(|place| TextHighlight::new(place.clone(), PaletteColor::WarningMuted).radius(radius))
    .collect()
}

/// The search of the messages of `chats` for `needle` in the project at `root`; what it finds goes to `outcome`.
#[derive(Clone)]
pub(super) struct RunProps {
  pub(super) root: Arc<Path>,
  pub(super) chats: Searched,
  pub(super) needle: Option<Needle>,
  pub(super) outcome: Signal<Outcome>,
}

/// Compared by what it searches; the signal is the screen's, the same for its life.
impl PartialEq for RunProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.root, &self.chats, &self.needle, self.outcome.id())
      == (&other.root, &other.chats, &other.needle, other.outcome.id())
  }
}

impl DevtoolsInspectable for RunProps {}

/// Searches the messages off the UI thread and hands what it found to History; it shows nothing itself. A new query
/// or conversation list starts the search over, and what the earlier one was still reading is no longer shown.
pub(super) struct SearchRun;

impl Component for SearchRun {
  type Props = RunProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunProps>().clone();
    let state = ctx
      .stream(
        (props.root.clone(), props.chats.clone(), props.needle.clone()),
        |(root, chats, needle), emitter: StreamEmitter<Outcome, ()>| run(root, chats, needle, emitter),
      )
      .state()
      .get();
    if let Some(outcome) = state.data
      && props.outcome.get_untracked() != outcome
    {
      props.outcome.set(outcome);
    }
    Column::new()
  }
}

/// Searches the messages of `chats` for `needle` a batch at a time, handing on what was found after each.
async fn run(root: Arc<Path>, chats: Searched, needle: Option<Needle>, emitter: StreamEmitter<Outcome, ()>) {
  let mut outcome = Outcome {
    needle: needle.clone(),
    chats: chats.clone(),
    ..Outcome::default()
  };
  let Some(needle) = needle else {
    emitter.emit(outcome);
    return;
  };
  tokio::time::sleep(TYPING_PAUSE).await;
  let mut found = HashMap::new();
  for batch in chats.chunks(BATCH) {
    let ids: Vec<ChatId> = batch.iter().map(|(chat, _)| *chat).collect();
    let (root, needle) = (root.clone(), needle.clone());
    match background::run(move || search_in(&root, &ids, &needle, &plain::rendered))
      .await
      .and_then(|searched| searched)
    {
      Ok(searched) => found.extend(searched.into_iter().map(|messages| (messages.chat, messages))),
      Err(reason) => outcome.failed = Some(reason),
    }
    outcome.done += batch.len();
    outcome.found = Arc::new(found.clone());
    let stopped = outcome.failed.is_some();
    if !emitter.emit(outcome.clone()) || stopped {
      return;
    }
  }
  if chats.is_empty() {
    emitter.emit(outcome);
  }
}
