//! Following the run live (`docs/design/orchester-workspace.md`, "Following live (ORC-36)"): the chat keeps
//! its newest row in view while it follows, or the card that waits for the person until the person scrolls;
//! any scroll up stops following and shows *Jump to latest · N new* (`Jump to latest` `tvvRD`: a 30 pill on
//! `surface_raised` in a `border_strong` stroke, its 13 `arrow-down` and 12/500 words 6 apart, 12 from its
//! sides, over a 0 4 16 shadow), floating centered over the chat's foot, *N* counting what arrived since; pressing it,
//! or sending a message ([`Follow::follow_again`]), goes to the end and follows again.

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
use orchester_workspace::tasks::timeline::{self, Item, view::Shown};
use parking_lot::Mutex;

use super::RowProps;
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

/// The card the chat reveals, and whether the person has scrolled since the tab opened.
#[derive(Default)]
struct Reveal {
  key: Option<String>,
  scrolled: bool,
}

/// Whether the chat follows the run, and what it needs to: its scroll, what had arrived when it stopped
/// following (for *N new*) and at the last render, whether it has shown rows yet, whether it is going to the
/// end after something arrived, how tall its content was at the last layout, and the card it reveals instead
/// of its end. Only arrivals move a following chat: a row the person opens or folds leaves it where it is.
#[derive(Clone)]
pub(in super::super) struct Follow {
  pub(in super::super) following: Signal<bool>,
  scroll: ScrollState,
  left_at: Arc<Mutex<Option<usize>>>,
  seen: Arc<Mutex<usize>>,
  started: Arc<Mutex<bool>>,
  /// Set when it goes to the end, until a layout no longer grows its content.
  to_end: Arc<Mutex<bool>>,
  height: Arc<Mutex<f32>>,
  reveal: Arc<Mutex<Reveal>>,
}

impl PartialEq for Follow {
  /// The tab's own handles, the same for as long as the tab is mounted.
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.started, &other.started)
  }
}

impl lurq::app::component::DevtoolsInspectable for Follow {}

impl Follow {
  pub(in super::super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      following: ctx.signal(true),
      scroll: ScrollState::new(),
      left_at: Arc::default(),
      seen: Arc::default(),
      started: Arc::default(),
      to_end: Arc::default(),
      height: Arc::default(),
      reveal: Arc::default(),
    }
  }

  /// Goes to the end of the chat and follows it again: *Jump to latest*, and a message the person sent.
  pub(in crate::screens::task::conversation) fn follow_again(&self) {
    *self.reveal.lock() = Reveal {
      key: None,
      scrolled: true,
    };
    self.to_end();
    self.following.set(true);
  }

  /// A row the person opens or folds: the chat stays where it is, its scroll held over the content changing
  /// under it, and stops following, so nothing moves under the pointer.
  pub(super) fn hold_in_place(&self) {
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

  /// Keeps the end of the chat in view while following: a render after something arrived (`arrived` counts
  /// what the run brought so far) goes to the end. The first time, the chat starts at the end; until the
  /// person scrolls, the first card that waits for them is revealed instead, for as long as it waits.
  pub(super) fn follow(&self, shown: &Shown, items: &[Item], arrived: usize) -> Option<String> {
    let rows = shown.rows.len();
    let came = std::mem::replace(&mut *self.seen.lock(), arrived) != arrived;
    let mut started = self.started.lock();
    let mut reveal = self.reveal.lock();
    let held = reveal.key.is_some();
    if !reveal.scrolled {
      reveal.key = items
        .iter()
        .find(|item| timeline::waits_for_you(item))
        .map(|item| item.key.to_string());
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

  /// `list`, scrolled as it follows: any scroll up stops following, a scroll down back to the end follows
  /// again. `arrived` counts what the run has brought so far, for *N new*.
  pub(super) fn list<'a>(
    &self,
    list: VirtualizedList<'a, RowProps>,
    arrived: usize,
    reveal: Option<String>,
  ) -> VirtualizedList<'a, RowProps> {
    let (scroll, following, left_at, revealed) = (
      self.scroll.clone(),
      self.following.clone(),
      self.left_at.clone(),
      self.reveal.clone(),
    );
    list
      .with_scroll_state(self.scroll.clone())
      .reveal_key(reveal)
      .on_scroll(move |event: ScrollEvent| {
        // The person scrolls: the card the tab opened at no longer holds the chat.
        *revealed.lock() = Reveal {
          key: None,
          scrolled: true,
        };
        // The handler runs before the wheel moves the list, which moves by the opposite of the delta: a
        // positive delta scrolls toward the start.
        let end = (scroll.content_height() - scroll.viewport_height()).max(0.0);
        let at_end = event.delta_y <= 0.0 && scroll.scroll_y() - event.delta_y >= end - FOLLOW_SLACK;
        if at_end != following.get_untracked() {
          *left_at.lock() = (!at_end).then_some(arrived);
          following.set(at_end);
        }
      })
  }

  /// *Jump to latest · N new*: what arrived since the chat stopped following.
  pub(super) fn jump(&self, ctx: &Ctx, arrived: usize) -> Element {
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

  /// What holds a following chat at its end, mounted after the list ([`FollowEnd`]).
  pub(super) fn end(&self, ctx: &mut Ctx) -> Element {
    ctx.mount_keyed::<FollowEnd>("conversation-follow-end", self.clone())
  }

  /// A chat going to its end after an arrival, whose content grew since the last layout, goes there at once,
  /// before the next frame: the list puts the row it anchored back where it was, which leaves the newest rows
  /// under the view. A layout that did not grow the content ends that; a row the person opened never starts
  /// it, so the scroll stays as the person left it.
  fn keep_at_end(&self) {
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

/// Mounted after the chat's list, so that its `after_layout` runs after the list's, which moves the scroll.
pub(super) struct FollowEnd {
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
