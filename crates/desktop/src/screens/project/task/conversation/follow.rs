//! Following the run live (`docs/design/orchester-workspace.md`, "Following live (ORC-36)"): the
//! virtualized timeline keeps its newest row in view while its end is in view, or the card that
//! waits for the person until the person scrolls; scrolling up stops following and shows *Jump to
//! latest · N new*; sending a message follows again. Rows that arrive or change while the person reads
//! further up leave what they read where it is: the list keeps the row at the top of its view in place.

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx, events::scroll_event::ScrollEvent, theme::SpacingSize},
  components::{Column, Row, VirtualizedList},
  core::Signal,
  layout::layout_kind::{Justify, ScrollState},
  node::{Element, dimension::Dimension},
};
use parking_lot::Mutex;

use super::{
  super::activity::{LiveFooterProps, RequestTarget, RevealRow},
  ConversationTab, ShowChoice,
  approval_card::AnswersLayout,
  row::{Line, RowProps, TimelineRowView},
};
use crate::{
  tasks::timeline::{
    self, Item, ItemKind,
    view::{self, Row as ChatRow, Shown},
  },
  theme::gutter_scrollbar,
  ui::{button::ActionButton, icon::Icon},
};

/// *Jump to latest* (`iwe3p`) and how close to the end still counts as at the end.
const JUMP_HEIGHT: f32 = 30.0;
const FOLLOW_SLACK: f32 = 24.0;
/// How far from the end a following timeline may sit before it goes to the end again.
const AT_END: f32 = 0.5;
/// The list's reach beyond its view while it measures its rows: all of them.
const MEASURE_ALL: f32 = f32::MAX;
/// The layouts that measure every row once the list shows rows: the list lays out a first batch of rows
/// to learn their size, then every row.
const MEASURING_LAYOUTS: u8 = 2;

/// How many rows the timeline showed when it stopped following, for *N new*; `None` until the
/// render after a step was picked, which shows other rows than before without any arriving.
#[derive(Default)]
pub(super) struct LeftAt(pub(super) Option<usize>);

/// The card the timeline reveals, and whether the person has scrolled since the tab opened.
#[derive(Default)]
pub(super) struct Reveal {
  key: Option<String>,
  scrolled: bool,
  /// A row a link asked for, held back until the list has laid out rows that hold it: rows that changed
  /// in the same render (*Tools* giving way to *All*) are not measured yet, and the list reveals a key
  /// only once.
  asked: Option<Asked>,
}

/// A row a link asked to reveal, and whether the rows shown hold it.
struct Asked {
  key: String,
  shown: bool,
}

impl Reveal {
  /// The person moved the chat (scrolled, jumped to the latest, sent a message or picked a step): no
  /// row holds it any more. The list reveals a row only when its key changes, so a row revealed before
  /// must be let go for a later reveal of the same row to scroll again.
  pub(super) fn let_go(reveal: &Mutex<Self>) {
    *reveal.lock() = Self {
      key: None,
      scrolled: true,
      asked: None,
    };
  }
}

impl ConversationTab {
  /// Once a message was sent, the timeline goes to its end and follows again.
  pub(super) fn sent_handler(&self) -> Arc<dyn Fn() + Send + Sync> {
    let (scroll, following, reveal) = (self.scroll.clone(), self.following.clone(), self.reveal.clone());
    Arc::new(move || {
      Reveal::let_go(&reveal);
      scroll.scroll_to_bottom_pending();
      following.set(true);
    })
  }

  /// Keeps the end of the timeline in view while following: the tab renders again whenever what it
  /// shows changes (a row added, a step folded as the next one starts, a group grown, the working
  /// line shown), so every render with `rows` to show goes to the end; counting rows would miss a
  /// fold that leaves as many rows as before. The first time, the timeline starts at the end; until
  /// the person scrolls, the first card that waits for them is the list's reveal instead, from the
  /// render it appears in (the run's state may read before its question) until it no longer waits.
  /// The key stays the same on every render: a reveal that changes to `None` is dropped before the
  /// list has measured its way to the card.
  pub(super) fn follow(&self, shown: &Shown, items: &[Item]) -> Option<String> {
    let rows = shown.rows.len();
    let mut started = self.started.lock();
    let mut reveal = self.reveal.lock();
    if let Some(asked) = &mut reveal.asked {
      asked.shown = shown.rows.iter().any(|row| **row.key() == *asked.key);
    }
    if !reveal.scrolled {
      reveal.key = items
        .iter()
        .find(|item| timeline::waits_for_you(item))
        .map(|item| item.key.to_string());
    }
    if (reveal.key.is_none() || !*started) && self.following.get_untracked() && rows > 0 {
      self.scroll.scroll_to_bottom_pending();
    }
    *started |= rows > 0;
    reveal.key.clone()
  }

  /// What holds the timeline at its end while it follows, mounted after the list ([`FollowEnd`]).
  pub(super) fn follow_end(&self, ctx: &mut Ctx) -> Element {
    ctx.mount_keyed::<FollowEnd>(
      "conversation-follow-end",
      EndHold {
        scroll: self.scroll.clone(),
        following: self.following.clone(),
        started: self.started.clone(),
        reveal: self.reveal.clone(),
        measured: self.measured.clone(),
      },
    )
  }

  /// After each layout: a row a link asked for is revealed once the list has laid out rows that hold it.
  pub(super) fn reveal_asked(&self) {
    let mut reveal = self.reveal.lock();
    if !reveal.asked.as_ref().is_some_and(|asked| asked.shown) {
      return;
    }
    reveal.key = reveal.asked.take().map(|asked| asked.key);
    drop(reveal);
    self.revealed.update(|count| *count += 1);
  }

  /// The timeline's rows, then `working`, the working line while an agent works.
  pub(super) fn list(
    &self,
    ctx: &mut Ctx,
    (shown, items): (&Shown, &[Item]),
    reveal: Option<String>,
    (working, answers): (Option<LiveFooterProps>, AnswersLayout),
  ) -> Element {
    let raw = self.raw.get();
    let rows: Arc<Vec<RowProps>> = Arc::new(
      shown
        .rows
        .iter()
        .map(|row| (Line::Row(Box::new(row.clone())), request_target(row, items)))
        .chain(working.map(|working| (Line::Working(working), None)))
        .map(|(line, request)| RowProps {
          line,
          raw,
          answers,
          request,
        })
        .collect(),
    );
    let count = shown.rows.len();
    let (scroll, following, left_at) = (self.scroll.clone(), self.following.clone(), self.left_at.clone());
    let revealed = self.reveal.clone();
    let list = VirtualizedList::new_shared(ctx, rows);
    // Until it has laid out every row once, the list sizes the rows it has not laid out by an estimate.
    // When rows arrive, it lays out every row it only estimated, and for one frame the rows in view
    // move by how far its estimates were off; laid out at once, while it shows its end, no row is left
    // to estimate but the ones that arrive under the view.
    let list = if self.measured.get() {
      list
    } else {
      list.overscan_px(MEASURE_ALL)
    };
    list
      .list_key("conversation-timeline")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .with_scroll_state(self.scroll.clone())
      .scrollbar(gutter_scrollbar())
      .reveal_key(reveal)
      .on_scroll(move |event: ScrollEvent| {
        // The person scrolls: the card the tab opened at no longer holds the list.
        Reveal::let_go(&revealed);
        // The handler runs before the wheel moves the list, which moves by the opposite of the delta.
        let end = (scroll.content_height() - scroll.viewport_height()).max(0.0);
        let at_end = scroll.scroll_y() - event.delta_y >= end - FOLLOW_SLACK;
        if at_end != following.get_untracked() {
          if !at_end {
            *left_at.lock() = LeftAt(Some(count));
          }
          following.set(at_end);
        }
      })
      .mount_keyed::<TimelineRowView, _, _, _>(|row| row.key().to_string(), Clone::clone)
  }

  /// Shows a request's card and scrolls the chat to it once, as when the tab reveals the card that waits
  /// for the person: *Tools* gives way to *All*, a search is cleared and the card's step opens, so the
  /// card is a row; the chat stops following, and the person's next scroll lets the row go.
  pub(super) fn reveal_row(&self) -> RevealRow {
    let (reveal, following, revealed) = (self.reveal.clone(), self.following.clone(), self.revealed.clone());
    let (show, query, toggles) = (self.show.clone(), self.query.clone(), self.toggles.clone());
    Arc::new(move |target: &RequestTarget| {
      if show.get_untracked().0 == view::Show::Tools {
        show.set(ShowChoice(view::Show::All));
      }
      if !query.get_untracked().is_empty() {
        query.set(String::new());
      }
      if let Some(step) = &target.step {
        toggles.open_all(std::slice::from_ref(step));
      }
      *reveal.lock() = Reveal {
        key: None,
        scrolled: true,
        asked: Some(Asked {
          key: target.card.to_string(),
          shown: false,
        }),
      };
      following.set(false);
      revealed.update(|count| *count += 1);
    })
  }

  /// "Jump to latest · N new" (`iwe3p`), centred above the composer.
  pub(super) fn jump(&self, ctx: &Ctx, rows: usize) -> Element {
    // Only rows that arrived since it stopped following are new; after a step was picked, the
    // rows it shows are where the count starts.
    let new = {
      let mut left_at = self.left_at.lock();
      let since = *left_at.0.get_or_insert(rows);
      rows.saturating_sub(since)
    };
    let label = if new == 0 {
      ctx.t("runs.activity.jump")
    } else {
      ctx.t_args("runs.activity.jumpNew", [("count", new.to_string().as_str())])
    };
    let (scroll, following, reveal) = (self.scroll.clone(), self.following.clone(), self.reveal.clone());
    let button = ActionButton::new(label)
      .icon(Icon::ArrowDown)
      .id("conversation-jump")
      .height(Dimension::Px(JUMP_HEIGHT))
      .on_click(move || {
        Reveal::let_go(&reveal);
        scroll.scroll_to_bottom_pending();
        following.set(true);
      })
      .render(ctx);
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding_bottom(SpacingSize::Md)
      .justify(Justify::Center)
      .child(button)
      .into()
  }
}

/// What holds a following timeline at its end: the list's scroll, whether it follows, whether it has
/// shown rows yet, and the card it reveals instead; and whether the list has laid out all its rows.
#[derive(Clone)]
pub(super) struct EndHold {
  scroll: ScrollState,
  following: Signal<bool>,
  started: Arc<Mutex<bool>>,
  reveal: Arc<Mutex<Reveal>>,
  measured: Signal<bool>,
}

impl PartialEq for EndHold {
  /// The tab's own handles, the same for as long as the tab is mounted.
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.started, &other.started) && Arc::ptr_eq(&self.reveal, &other.reveal)
  }
}

impl lurq::app::component::DevtoolsInspectable for EndHold {}

impl EndHold {
  /// A timeline that follows and has come short of its end goes to it at once, before the next frame
  /// is laid out: as rows arrive, the list puts the row it anchored back where it was, which leaves the
  /// newest rows under the view. Going there at once lets the list build the rows of the end, so no
  /// frame shows anything but the end while *Following live* is on.
  fn keep_at_end(&self) {
    if !self.following.get_untracked() || !*self.started.lock() || self.reveal.lock().key.is_some() {
      return;
    }
    let end = (self.scroll.content_height() - self.scroll.viewport_height()).max(0.0);
    if self.scroll.scroll_y() < end - AT_END {
      self.scroll.set_scroll(0.0, end);
      // Rows the next layout measures taller than the list estimated them move the end further.
      self.scroll.scroll_to_bottom_pending();
    }
  }
}

/// Mounted after the timeline's list, and with it, so that its `after_layout` runs after the list's,
/// which moves the scroll: the components the tab mounted run theirs after the tab's own, in the order
/// they were mounted. It counts the layouts the list measures every row in.
pub(super) struct FollowEnd {
  hold: Mutex<Option<EndHold>>,
  layouts: Mutex<u8>,
}

impl Component for FollowEnd {
  type Props = EndHold;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      hold: Mutex::new(None),
      layouts: Mutex::new(0),
    }
  }

  fn after_layout(&self) {
    let Some(hold) = &*self.hold.lock() else {
      return;
    };
    hold.keep_at_end();
    if hold.measured.get_untracked() {
      return;
    }
    let mut layouts = self.layouts.lock();
    *layouts += 1;
    if *layouts >= MEASURING_LAYOUTS {
      hold.measured.set(true);
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    *self.hold.lock() = Some(ctx.props::<EndHold>().clone());
    Column::new()
  }
}

/// The card of the tool request problem row `row` names and its step, when `items`, the run's timeline,
/// hold the card.
fn request_target(row: &ChatRow, items: &[Item]) -> Option<RequestTarget> {
  let entry = match row {
    ChatRow::Item(Item {
      kind: ItemKind::Problem(entry),
      ..
    })
    | ChatRow::Entry { entry, .. } => entry,
    _ => return None,
  };
  let card = timeline::named_request(entry)?;
  items.iter().any(|item| item.key == card).then(|| RequestTarget {
    step: timeline::step_of(items, &card),
    card,
  })
}
