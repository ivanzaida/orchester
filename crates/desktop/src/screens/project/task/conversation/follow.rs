//! Following the run live (`docs/design/orchester-workspace.md`, "Following live (ORC-36)"): the
//! virtualized timeline keeps its newest row in view while its end is in view, or the card that
//! waits for the person until the person scrolls; scrolling up stops following and shows *Jump to
//! latest · N new*; sending a message follows again.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, events::scroll_event::ScrollEvent, theme::SpacingSize},
  components::{Row, VirtualizedList},
  layout::layout_kind::Justify,
  node::{Element, dimension::Dimension},
};

use super::{
  super::activity::LiveFooterProps,
  ConversationTab,
  approval_card::AnswersLayout,
  row::{Line, RowProps, TimelineRowView},
};
use crate::{
  tasks::timeline::{self, Item, view::Shown},
  theme::gutter_scrollbar,
  ui::{button::ActionButton, icon::Icon},
};

/// *Jump to latest* (`iwe3p`) and how close to the end still counts as at the end.
const JUMP_HEIGHT: f32 = 30.0;
const FOLLOW_SLACK: f32 = 24.0;
/// How far from the end a following timeline may sit before it goes to the end again.
const AT_END: f32 = 0.5;

/// How many rows the timeline showed when it stopped following, for *N new*; `None` until the
/// render after a step was picked, which shows other rows than before without any arriving.
#[derive(Default)]
pub(super) struct LeftAt(pub(super) Option<usize>);

/// The card the timeline reveals, and whether the person has scrolled since the tab opened.
#[derive(Default)]
pub(super) struct Reveal {
  key: Option<String>,
  scrolled: bool,
}

impl ConversationTab {
  /// Once a message was sent, the timeline goes to its end and follows again.
  pub(super) fn sent_handler(&self) -> Arc<dyn Fn() + Send + Sync> {
    let (scroll, following) = (self.scroll.clone(), self.following.clone());
    Arc::new(move || {
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
  pub(super) fn follow(&self, rows: usize, items: &[Item]) -> Option<String> {
    let mut started = self.started.lock();
    let mut reveal = self.reveal.lock();
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

  /// After each layout: a timeline that follows and has come short of its end (rows measured taller
  /// than the list estimated them, a row that grew after the render went to the end) goes to it
  /// again, so *Following live* never shows anything but the end.
  pub(super) fn keep_at_end(&self) {
    if !self.following.get_untracked() || !*self.started.lock() || self.reveal.lock().key.is_some() {
      return;
    }
    let end = (self.scroll.content_height() - self.scroll.viewport_height()).max(0.0);
    if self.scroll.scroll_y() < end - AT_END {
      self.scroll.scroll_to_bottom_pending();
    }
  }

  /// The timeline's rows, then `working`, the working line while an agent works.
  pub(super) fn list(
    &self,
    ctx: &mut Ctx,
    shown: &Shown,
    reveal: Option<String>,
    (working, answers): (Option<LiveFooterProps>, AnswersLayout),
  ) -> Element {
    let raw = self.raw.get();
    let rows: Arc<Vec<RowProps>> = Arc::new(
      shown
        .rows
        .iter()
        .map(|row| Line::Row(Box::new(row.clone())))
        .chain(working.map(Line::Working))
        .map(|line| RowProps { line, raw, answers })
        .collect(),
    );
    let count = shown.rows.len();
    let (scroll, following, left_at) = (self.scroll.clone(), self.following.clone(), self.left_at.clone());
    let revealed = self.reveal.clone();
    VirtualizedList::new_shared(ctx, rows)
      .list_key("conversation-timeline")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .with_scroll_state(self.scroll.clone())
      .scrollbar(gutter_scrollbar())
      .reveal_key(reveal)
      .on_scroll(move |event: ScrollEvent| {
        // The person scrolls: the card the tab opened at no longer holds the list.
        *revealed.lock() = Reveal {
          key: None,
          scrolled: true,
        };
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
    let (scroll, following) = (self.scroll.clone(), self.following.clone());
    let button = ActionButton::new(label)
      .icon(Icon::ArrowDown)
      .id("conversation-jump")
      .height(Dimension::Px(JUMP_HEIGHT))
      .on_click(move || {
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
