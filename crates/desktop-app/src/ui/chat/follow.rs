//! Following a chat live, a task's Conversation and the project's Orchestrator chat alike
//! (`docs/design/orchester-workspace.md`, "Following live (ORC-36)"): the chat opens at the very end of its newest
//! row and keeps it in view while it follows, or the card that waits for the person until the person scrolls; any
//! scroll that moves the chat up (not a box that scrolls inside a row) stops following and shows *Jump to latest ·
//! N new* (`Jump to latest` `tvvRD`: a 30 pill on `surface_raised` in a `border_strong` stroke, its 13 `arrow-down`
//! and 12/500 words 6 apart, 12 from its sides, over a 0 4 16 shadow), floating centered over the chat's foot, *N*
//! counting what arrived since; pressing it, or sending a message ([`Follow::follow_again`]), goes to the end and
//! follows again.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::scroll_event::ScrollEvent,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, VirtualizedList},
  core::Signal,
  layout::layout_kind::ScrollState,
  node::{Element, border::Border, dimension::Dimension},
};
use parking_lot::Mutex;

use crate::{
  theme::{ConversationShadow, ExtraRadius, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// *Jump to latest*'s height (`tvvRD`).
const JUMP_HEIGHT: f32 = 30.0;
/// How close to the end a scroll down counts as back at the end.
const FOLLOW_SLACK: f32 = 24.0;
/// How far from the end a following chat may sit, or its content grow, before it goes to the end again.
const AT_END: f32 = 0.5;

/// The card the chat reveals, and whether the person has scrolled since the chat opened.
#[derive(Default)]
struct Reveal {
  key: Option<String>,
  scrolled: bool,
}

/// A turn of the wheel over the chat, until the layout after it shows whether it moved the chat: a wheel over a
/// box that scrolls inside a row (an attached file's text) moves that box and leaves the chat where it is.
#[derive(Clone, Copy)]
struct Wheel {
  /// Toward the start.
  up: bool,
  /// The chat's scroll before it.
  from: f32,
  /// What had arrived then, for *N new*.
  arrived: usize,
}

/// Whether the chat follows, and what it needs to: its scroll, what had arrived when it stopped following (for
/// *N new*) and at the last render, whether it has shown rows yet, whether it is going to the end after something
/// arrived, how tall its content was at the last layout, the card it reveals instead of its end, and which
/// conversation it shows when one chat shows several, and the turn of the wheel the next layout settles. Only
/// arrivals move a following chat: a row the person opens or folds leaves it where it is.
#[derive(Clone)]
pub struct Follow {
  pub following: Signal<bool>,
  scroll: ScrollState,
  left_at: Arc<Mutex<Option<usize>>>,
  seen: Arc<Mutex<usize>>,
  started: Arc<Mutex<bool>>,
  /// Set when it goes to the end, until a layout no longer grows its content.
  to_end: Arc<Mutex<bool>>,
  height: Arc<Mutex<f32>>,
  reveal: Arc<Mutex<Reveal>>,
  shows: Arc<Mutex<Option<String>>>,
  wheel: Arc<Mutex<Option<Wheel>>>,
}

impl PartialEq for Follow {
  /// The chat's own handles, the same for as long as the chat is mounted.
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.started, &other.started)
  }
}

impl lurq::app::component::DevtoolsInspectable for Follow {}

impl Follow {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      following: ctx.signal(true),
      scroll: ScrollState::new(),
      left_at: Arc::default(),
      seen: Arc::default(),
      started: Arc::default(),
      to_end: Arc::default(),
      height: Arc::default(),
      reveal: Arc::default(),
      shows: Arc::default(),
      wheel: Arc::default(),
    }
  }

  /// Goes to the end of the chat and follows it again: *Jump to latest*, and a message the person sent.
  pub fn follow_again(&self) {
    *self.reveal.lock() = Reveal {
      key: None,
      scrolled: true,
    };
    *self.wheel.lock() = None;
    self.to_end();
    self.following.set(true);
  }

  /// The chat shows `conversation`: another one than at the last render opens as a chat just mounted does, at its
  /// end, following.
  pub fn shows(&self, conversation: &str) {
    {
      let mut shows = self.shows.lock();
      if shows.as_deref() == Some(conversation) {
        return;
      }
      *shows = Some(conversation.to_owned());
    }
    *self.reveal.lock() = Reveal::default();
    *self.started.lock() = false;
    *self.left_at.lock() = None;
    *self.height.lock() = 0.0;
    *self.to_end.lock() = false;
    *self.wheel.lock() = None;
    if !self.following.get_untracked() {
      self.following.set(true);
    }
  }

  /// A row the person opens or folds: the chat stays where it is, its scroll held over the content changing
  /// under it, and stops following, so nothing moves under the pointer.
  pub fn hold_in_place(&self) {
    self
      .scroll
      .set_scroll_pending(self.scroll.scroll_x(), self.scroll.scroll_y());
    *self.to_end.lock() = false;
    if self.following.get_untracked() {
      *self.left_at.lock() = Some(*self.seen.lock());
      self.following.set(false);
    }
  }

  fn to_end(&self) {
    self.scroll.scroll_to_bottom_pending();
    *self.to_end.lock() = true;
  }

  /// Keeps the end of the chat in view while following: a render after something arrived (`arrived` counts what
  /// the chat brought so far) goes to the end. The first time it has `rows`, the chat starts at the end; until the
  /// person scrolls, the card `waiting` for them (its row's key) is revealed instead, for as long as it waits:
  /// the key of the row to reveal.
  pub fn follow(&self, rows: usize, arrived: usize, waiting: Option<String>) -> Option<String> {
    let came = std::mem::replace(&mut *self.seen.lock(), arrived) != arrived;
    let mut started = self.started.lock();
    let mut reveal = self.reveal.lock();
    let held = reveal.key.is_some();
    if !reveal.scrolled {
      reveal.key = waiting;
    }
    // The first rows start at the end; later, only an arrival moves the chat, unless a card holds it.
    // A card the chat held stopped waiting before the person scrolled: the chat goes on to the end.
    let released = held && reveal.key.is_none();
    let goes = !*started || (reveal.key.is_none() && (came || released));
    if goes && self.following.get_untracked() && rows > 0 {
      self.to_end();
    }
    *started |= rows > 0;
    reveal.key.clone()
  }

  /// `list`, scrolled as it follows: any scroll up stops following, a scroll down back to the end follows again.
  /// `arrived` counts what the chat has brought so far, for *N new*; `reveal` is what [`Follow::follow`] reveals.
  pub fn list<'a, T>(
    &self,
    list: VirtualizedList<'a, T>,
    arrived: usize,
    reveal: Option<String>,
  ) -> VirtualizedList<'a, T> {
    let (scroll, wheel) = (self.scroll.clone(), self.wheel.clone());
    list
      .with_scroll_state(self.scroll.clone())
      .reveal_key(reveal)
      .on_scroll(move |event: ScrollEvent| {
        // The handler runs before the wheel moves anything, also for a wheel over a box that scrolls inside a row,
        // which moves that box instead: the layout after it tells (`settle_wheel`). The list moves by the opposite
        // of the delta: a positive delta scrolls toward the start.
        let mut wheel = wheel.lock();
        let from = wheel.map_or_else(|| scroll.scroll_y(), |earlier| earlier.from);
        *wheel = Some(Wheel {
          up: event.delta_y > 0.0,
          from,
          arrived,
        });
      })
  }

  /// A turn of the wheel that moved the chat: the card the chat opened at no longer holds it, a move up stops
  /// following, and a move down to the end follows again. One that moved only a box inside a row changes nothing.
  fn settle_wheel(&self) {
    let Some(wheel) = self.wheel.lock().take() else {
      return;
    };
    let now = self.scroll.scroll_y();
    if (now - wheel.from).abs() <= AT_END {
      return;
    }
    *self.reveal.lock() = Reveal {
      key: None,
      scrolled: true,
    };
    let end = (self.scroll.content_height() - self.scroll.viewport_height()).max(0.0);
    let at_end = !wheel.up && now >= end - FOLLOW_SLACK;
    if at_end != self.following.get_untracked() {
      *self.left_at.lock() = (!at_end).then_some(wheel.arrived);
      self.following.set(at_end);
    }
  }

  /// *Jump to latest · N new*: what arrived since the chat stopped following.
  pub fn jump(&self, ctx: &Ctx, arrived: usize) -> Element {
    let new = {
      let mut left_at = self.left_at.lock();
      arrived.saturating_sub(*left_at.get_or_insert(arrived))
    };
    let label = if new == 0 {
      ctx.t("conversation.jump")
    } else {
      ctx.t_args("conversation.jumpNew", [("count", new.to_string().as_str())])
    };
    let follow = self.clone();
    let button = pressable(move || follow.follow_again())
      .id("conversation-jump")
      .height(Dimension::Px(JUMP_HEIGHT))
      .padding_horizontal(SpacingSize::Md)
      .spacing(ExtraSpacing::Snug.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(ExtraRadius::Full.role())
      .box_shadow(ConversationShadow::Jump.role())
      .child(icon(Icon::ArrowDown, IconSize::Base, PaletteColor::TextSecondary))
      .child(Text::new(&label).variant(TypographyStyle::Label).nowrap());
    Row::new().padding_bottom(SpacingSize::Sm).child(button).into()
  }

  /// What holds a following chat at its end and settles the person's wheel, mounted after the list
  /// ([`FollowEnd`]); `key` names it.
  pub fn end(&self, ctx: &mut Ctx, key: &str) -> Element {
    ctx.mount_keyed::<FollowEnd>(key, self.clone())
  }

  /// A chat going to its end after an arrival, whose content grew since the last layout, goes there at once,
  /// before the next frame: the list puts the row it anchored back where it was, which leaves the newest rows
  /// under the view, and rows it measures late (Markdown, cards) move the end further. A layout that did not grow
  /// the content ends that; a row the person opened never starts it, so the scroll stays as the person left it.
  fn keep_at_end(&self) {
    self.settle_wheel();
    let height = self.scroll.content_height();
    let grew = {
      let mut last = self.height.lock();
      let grew = height > *last + AT_END;
      *last = height;
      grew
    };
    let mut to_end = self.to_end.lock();
    if !grew {
      *to_end = false;
    }
    if !*to_end || !self.following.get_untracked() || self.reveal.lock().key.is_some() {
      return;
    }
    let end = (height - self.scroll.viewport_height()).max(0.0);
    if self.scroll.scroll_y() < end - AT_END {
      self.scroll.set_scroll(0.0, end);
      // Rows the next layout measures taller than the list estimated them move the end further.
      self.scroll.scroll_to_bottom_pending();
    }
  }
}

/// The room *Jump to latest* takes over the chat's foot, under the last row while it shows: the pill's height
/// (the row's own padding adds the gap).
pub fn jump_room() -> Element {
  Column::new().height(Dimension::Px(JUMP_HEIGHT)).into()
}

/// Mounted after the chat's list, so that its `after_layout` runs after the list's, which moves the scroll, and after
/// a wheel has moved what it moves.
struct FollowEnd {
  follow: Mutex<Option<Follow>>,
}

impl Component for FollowEnd {
  type Props = Follow;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      follow: Mutex::new(None),
    }
  }

  fn after_layout(&self) {
    if let Some(follow) = &*self.follow.lock() {
      follow.keep_at_end();
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    *self.follow.lock() = Some(ctx.props::<Follow>().clone());
    Column::new()
  }
}
