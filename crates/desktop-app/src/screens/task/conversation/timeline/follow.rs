//! Following the run live (`docs/design/orchester-workspace.md`, "Following live (ORC-36)"): the chat keeps
//! its newest row in view while its end is in view, or the card that waits for the person until the person
//! scrolls; scrolling up stops following and shows *Jump to latest · N new* (`Jump to latest` `tvvRD`: a 30
//! pill on `surface_raised` in a `border_strong` stroke, its 13 `arrow-down` and 12/500 words 6 apart, 12 from
//! its sides, over a 0 4 16 shadow), centered under the chat; pressing it goes to the end and follows again.

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
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
  },
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

/// *Jump to latest*'s height (`tvvRD`), and how close to the end still counts as at the end.
const JUMP_HEIGHT: f32 = 30.0;
const FOLLOW_SLACK: f32 = 24.0;
/// How far from the end a following chat may sit before it goes to the end again.
const AT_END: f32 = 0.5;
/// The list's reach beyond its view while it measures its rows: all of them.
const MEASURE_ALL: f32 = f32::MAX;
/// The layouts that measure every row once the list shows rows: a first batch to learn their size, then
/// every row.
const MEASURING_LAYOUTS: u8 = 2;

/// The card the chat reveals, and whether the person has scrolled since the tab opened.
#[derive(Default)]
struct Reveal {
  key: Option<String>,
  scrolled: bool,
}

/// Whether the chat follows the run, and what it needs to: its scroll, how many rows it showed when it
/// stopped following (for *N new*), whether it has shown rows yet, whether it has measured all of them, and
/// the card it reveals instead of its end.
#[derive(Clone)]
pub(in super::super) struct Follow {
  pub(in super::super) following: Signal<bool>,
  scroll: ScrollState,
  left_at: Arc<Mutex<Option<usize>>>,
  started: Arc<Mutex<bool>>,
  measured: Signal<bool>,
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
      started: Arc::default(),
      measured: ctx.signal(false),
      reveal: Arc::default(),
    }
  }

  /// Keeps the end of the chat in view while following: the tab renders again whenever what it shows
  /// changes, so every render with rows goes to the end. The first time, the chat starts at the end; until the
  /// person scrolls, the first card that waits for them is revealed instead, for as long as it waits.
  pub(super) fn follow(&self, shown: &Shown, items: &[Item]) -> Option<String> {
    let rows = shown.rows.len();
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

  /// `list`, the chat's `count` rows, scrolled as it follows: scrolling up stops following, back at the end
  /// follows again.
  pub(super) fn list<'a>(
    &self,
    list: VirtualizedList<'a, RowProps>,
    count: usize,
    reveal: Option<String>,
  ) -> VirtualizedList<'a, RowProps> {
    // Until it has laid out every row once, the list sizes the rows it has not laid out by an estimate; laid
    // out at once, while it shows its end, no row is left to estimate but the ones that arrive under the view.
    let list = if self.measured.get() {
      list
    } else {
      list.overscan_px(MEASURE_ALL)
    };
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
        // The handler runs before the wheel moves the list, which moves by the opposite of the delta.
        let end = (scroll.content_height() - scroll.viewport_height()).max(0.0);
        let at_end = scroll.scroll_y() - event.delta_y >= end - FOLLOW_SLACK;
        if at_end != following.get_untracked() {
          if !at_end {
            *left_at.lock() = Some(count);
          }
          following.set(at_end);
        }
      })
  }

  /// *Jump to latest · N new*: only the rows that arrived since the chat stopped following are new.
  pub(super) fn jump(&self, ctx: &Ctx, rows: usize) -> Element {
    let new = {
      let mut left_at = self.left_at.lock();
      rows.saturating_sub(*left_at.get_or_insert(rows))
    };
    let label = if new == 0 {
      ctx.t("conversation.jump")
    } else {
      ctx.t_args("conversation.jumpNew", [("count", new.to_string().as_str())])
    };
    let (scroll, following, reveal) = (self.scroll.clone(), self.following.clone(), self.reveal.clone());
    let button = pressable(move || {
      *reveal.lock() = Reveal {
        key: None,
        scrolled: true,
      };
      scroll.scroll_to_bottom_pending();
      following.set(true);
    })
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
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Sm)
      .justify(Justify::Center)
      .align_items(Alignment::Center)
      .child(button)
      .into()
  }

  /// What holds a following chat at its end, mounted after the list ([`FollowEnd`]).
  pub(super) fn end(&self, ctx: &mut Ctx) -> Element {
    ctx.mount_keyed::<FollowEnd>("conversation-follow-end", self.clone())
  }

  /// A chat that follows and has come short of its end goes to it at once, before the next frame is laid
  /// out: as rows arrive, the list puts the row it anchored back where it was, which leaves the newest rows
  /// under the view.
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

/// Mounted after the chat's list, so that its `after_layout` runs after the list's, which moves the
/// scroll. It counts the layouts the list measures every row in.
pub(super) struct FollowEnd {
  follow: Mutex<Option<Follow>>,
  layouts: Mutex<u8>,
}

impl Component for FollowEnd {
  type Props = Follow;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      follow: Mutex::new(None),
      layouts: Mutex::new(0),
    }
  }

  fn after_layout(&self) {
    let Some(follow) = &*self.follow.lock() else {
      return;
    };
    follow.keep_at_end();
    if follow.measured.get_untracked() {
      return;
    }
    let mut layouts = self.layouts.lock();
    *layouts += 1;
    if *layouts >= MEASURING_LAYOUTS {
      follow.measured.set(true);
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    *self.follow.lock() = Some(ctx.props::<Follow>().clone());
    Column::new()
  }
}
