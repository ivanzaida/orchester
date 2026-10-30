//! The Browser window's tab strip (`Tab strip` `X1V8q`, `01c` `fPkxD`, `01d` `fWhIr`, `01e` `cmofl`): agents' tabs,
//! a thin divider, the person's tabs and *+*, then at least 48 of room that drags the window. The tabs share the room
//! as `fit` says; past their narrowest the strip scrolls sideways, keeps the tab shown in view and fades at a cut edge,
//! and the overflow button before *+* counts the tabs not wholly in view, with the strongest dot among them. It
//! measures its own room from its last layout.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::ScrollEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, ScrollHorizontal, Stack},
  core::{ElementRef, Signal},
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
    scrollbar::ScrollBarStyle,
  },
  node::{Element, Gradient, GradientStop, HitTestBehavior, Style, dimension::Dimension, padding::Padding},
};
use orchester_ipc::browser::{BrowserCommand, BrowserTabId};
use parking_lot::Mutex;

use super::{
  fit::{self, StripFit, gap},
  overflow::{MenuFocus, Overflow, OverflowProps},
  state::Showing,
  tab::{TAB_HEIGHT, Tab, TabProps},
};
use crate::{
  theme::{BrowserColor, DISABLED_OPACITY, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::{Focus, in_title_bar, pressable},
  },
};

/// *+* (`New tab` `n2KL9`), the room that always stays to drag the window, the divider's height, a cut edge's fade,
/// and the overflow button's room (`⌄ 13 •` in `fWhIr`) before it was laid out.
const NEW_TAB_WIDTH: f32 = 26.0;
const MIN_DRAG_WIDTH: f32 = 48.0;
const DIVIDER_HEIGHT: f32 = 16.0;
const FADE_WIDTH: f32 = 24.0;
const OVERFLOW_WIDTH: f32 = 52.0;

pub struct Strip {
  room: Signal<Option<f32>>,
  element: ElementRef,
  scroll: ScrollState,
  scrolled: Signal<f32>,
  /// The tab last scrolled into view.
  followed: Mutex<Option<BrowserTabId>>,
  menu_focus: MenuFocus,
}

impl Component for Strip {
  type Props = Showing;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      room: ctx.signal(None),
      element: ctx.element_ref(),
      scroll: ScrollState::new(),
      scrolled: ctx.signal(0.0),
      followed: Mutex::new(None),
      menu_focus: MenuFocus::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let showing = ctx.props::<Showing>().clone();
    let strip = &showing.strip;
    let shown = showing.shown().cloned();
    let shown_index = shown
      .as_ref()
      .and_then(|tab| strip.all().position(|other| &other.id == tab));
    let divider = (!strip.agents.is_empty() && !strip.person.is_empty()).then_some(strip.agents.len());
    let whole = self.measured_room();
    let reserve = gap() + NEW_TAB_WIDTH + gap() + MIN_DRAG_WIDTH;
    let mut room = (whole - reserve).max(0.0);
    let mut fitted = fit::fit(room, strip.len(), shown_index, divider.is_some());
    if fitted.overflows() {
      room = (room - OVERFLOW_WIDTH - gap()).max(0.0);
      fitted = fit::fit(room, strip.len(), shown_index, divider.is_some());
    }
    let content = fitted.content_width(divider);
    let visible = content.min(room);
    self.follow(&fitted, (shown.as_ref(), shown_index, divider), visible);
    let scroll = self.scrolled.get();
    let tabs = self.tabs(ctx, &showing, &fitted, (shown.as_ref(), divider));
    let viewport = self.viewport(tabs, visible, (scroll, content));
    let overflow = fitted.overflows().then(|| {
      let hidden = fitted.hidden(divider, (scroll, visible));
      let strongest = strip.strongest(&hidden);
      ctx.mount_keyed::<Overflow>(
        "browser-overflow",
        OverflowProps {
          hidden: hidden.len(),
          strongest,
          showing: showing.clone(),
          focus: self.menu_focus.clone(),
        },
      )
    });
    if overflow.is_none() {
      self.menu_focus.follow(ctx, &showing, false);
    }
    let strip = Row::new()
      .id("browser-strip")
      .ref_element(self.element.clone())
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(viewport)
      .with_children(overflow)
      .child(new_tab(ctx, &showing))
      .child(
        Row::new()
          .id("browser-drag")
          .width(Dimension::Px(MIN_DRAG_WIDTH))
          .height(Dimension::Px(TAB_HEIGHT)),
      )
      .child(Row::new().flex(1.0));
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(SpacingSize::Sm))
      .child(strip)
  }
}

impl Strip {
  /// The room the strip had at its last layout, watched from now on; the design's until it was laid out.
  fn measured_room(&self) -> f32 {
    let room = self.room.clone();
    self.element.observe_rect(move |rect| {
      if rect.width > 0.0
        && room
          .get_untracked()
          .is_none_or(|known| (known - rect.width).abs() > 0.5)
      {
        room.set(Some(rect.width));
      }
    });
    self.room.get().unwrap_or(fit::TAB_MAX_WIDTH * 4.0)
  }

  /// Scrolls the tab shown into view when it changes.
  fn follow(
    &self,
    fitted: &StripFit,
    (shown, index, divider): (Option<&BrowserTabId>, Option<usize>, Option<usize>),
    room: f32,
  ) {
    let (Some(shown), Some(index)) = (shown, index) else {
      return;
    };
    let mut followed = self.followed.lock();
    if followed.as_ref() == Some(shown) {
      return;
    }
    *followed = Some(shown.clone());
    let to = fitted.scroll_to(index, divider, (self.scroll.scroll_x(), room));
    self.scroll.set_scroll_pending(to, 0.0);
    self.scrolled.set(to);
  }

  fn tabs(
    &self,
    ctx: &mut Ctx,
    showing: &Showing,
    fitted: &StripFit,
    (shown, divider): (Option<&BrowserTabId>, Option<usize>),
  ) -> Row {
    let line = ctx.theme().border_size_value(BorderSize::Sm);
    let mut row = Row::new()
      .id("browser-tabs")
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center);
    for (index, tab) in showing.strip.all().enumerate() {
      if divider == Some(index) {
        row = row.child(
          Row::new()
            .id("browser-tabs-divider")
            .width(Dimension::Px(line))
            .height(Dimension::Px(DIVIDER_HEIGHT))
            .flex_shrink(0.0)
            .background(PaletteColor::Border),
        );
      }
      let props = TabProps {
        tab: tab.clone(),
        index,
        width: fitted.widths().get(index).copied().unwrap_or(fit::TAB_MIN_WIDTH),
        shown: shown == Some(&tab.id),
        showing: showing.clone(),
      };
      row = row.child(ctx.mount_keyed::<Tab>(&format!("browser-tab-{}", tab.id.as_str()), props));
    }
    row
  }

  /// The tabs in a viewport `visible` wide that scrolls sideways with the wheel, with a fade at each cut edge and no
  /// scrollbar. lurq's sideways scroller leaves a vertical wheel to the containers around it, so the strip scrolls
  /// itself by the stronger of the two deltas: a mouse wheel and a trackpad's sideways swipe both move the tabs.
  fn viewport(&self, tabs: Row, visible: f32, (scroll, content): (f32, f32)) -> Element {
    let (scrolled, state) = (self.scrolled.clone(), self.scroll.clone());
    let most = (content - visible).max(0.0);
    let scroller = ScrollHorizontal::new(tabs)
      .with_scroll_state(self.scroll.clone())
      .scrollbar(ScrollBarStyle::hidden())
      .width(Dimension::Px(visible))
      .height(Dimension::Px(TAB_HEIGHT))
      .on_scroll(move |event: ScrollEvent| {
        event.prevent_default();
        let delta = if event.delta_y.abs() > event.delta_x.abs() {
          event.delta_y
        } else {
          event.delta_x
        };
        let to = (scrolled.get_untracked() - delta).clamp(0.0, most);
        state.set_scroll_pending(to, 0.0);
        scrolled.set(to);
      });
    let fade = |id: &str, angle: f32, left: f32| {
      Row::new()
        .id(id.to_owned())
        .absolute(left, 0.0, Dimension::Px(FADE_WIDTH), Dimension::Px(TAB_HEIGHT))
        .hit_test(HitTestBehavior::None)
        .background_gradient(Gradient::linear(
          angle,
          [
            GradientStop::new(PaletteColor::SurfaceBase),
            GradientStop::new(BrowserColor::FadeClear.role()),
          ],
        ))
    };
    let start = (scroll > 0.5).then(|| fade("browser-tabs-fade-start", 90.0, 0.0));
    let end = (scroll + visible < content - 0.5).then(|| fade("browser-tabs-fade-end", 270.0, visible - FADE_WIDTH));
    Stack::new()
      .width(Dimension::Px(visible))
      .height(Dimension::Px(TAB_HEIGHT))
      .flex_shrink(0.0)
      .child(scroller)
      .with_children(start)
      .with_children(end)
      .into()
  }
}

/// *+* (`New tab` `n2KL9`): a tab of the person's in the shown project's browser; faded while no browser was found.
fn new_tab(ctx: &mut Ctx, showing: &Showing) -> Element {
  let commands = showing.commands.clone();
  let hooks = showing.person.clone();
  let unavailable = showing.unavailable();
  let button = in_title_bar(Focus::Pointer.apply(pressable(move || {
    if unavailable {
      return;
    }
    let hooks = hooks.clone();
    commands.send_then(BrowserCommand::OpenTab { url: None }, move |reply| {
      if let orchester_ipc::browser::BrowserReply::TabOpened { tab } = reply {
        hooks.pick(&tab);
      }
    });
  })))
  .id("browser-new-tab")
  .describe("a11y_name", ctx.t("browser.tabs.newTab"))
  .describe("state", if unavailable { "unavailable" } else { "enabled" })
  .width(Dimension::Px(NEW_TAB_WIDTH))
  .height(Dimension::Px(TAB_HEIGHT))
  .flex_shrink(0.0)
  .justify(Justify::Center)
  .rounded(RadiusSize::Md)
  .child(icon(Icon::Plus, IconSize::Md, PaletteColor::TextMuted));
  if unavailable {
    button.opacity(DISABLED_OPACITY).into()
  } else {
    button
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .into()
  }
}
